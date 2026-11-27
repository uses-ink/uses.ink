//! Removing a site, bytes before rows (decision 0022).

use blob::{Blob, Deleted};
use db::{Db, Statement};
use jiff::Timestamp;
use model::Bytes;

use crate::id::{Id, Site as SiteKind};
use crate::{Control, Error, Ports, Receipt};

impl<P: Ports> Control<P> {
    /// Removes a site and everything of it, and returns what was removed.
    ///
    /// The order is the point. First the site is marked `removing` and its live pointer is
    /// dropped, so it serves nothing. Then every blob of its tenant is deleted. Only then are
    /// its rows deleted, and its handle left as a tombstone. A crash between the steps leaves
    /// the marked row, which is a diagnosable state and never bytes without a row; removing the
    /// site again finishes the job, and the receipt counts only what that run removed of blobs.
    pub async fn remove_site(&self, id: Id<SiteKind>, at: Timestamp) -> Result<Receipt, Error<P>> {
        const COUNTS: &str = "SELECT (SELECT COUNT(*) FROM deploy WHERE site_id = ?) AS deploys, \
            (SELECT COUNT(*) FROM job WHERE site_id = ?) AS builds, \
            (SELECT COUNT(*) FROM credential WHERE site_id = ?) AS credentials";
        const MARK: &str = "UPDATE site SET state = 'removing' WHERE id = ?";
        const UNPUBLISH: &str = "DELETE FROM site_live WHERE site_id = ?";
        let site = self.site(id).await?;
        let tenant = Self::tenant(id)?;
        let db = self.ports.db();
        let key = id.to_string();
        let counts = db.first(Statement::prepare(COUNTS).bind(key.clone()).bind(key.clone()).bind(key.clone())).await?;
        let counts = counts.ok_or(db::ColumnError { column: "deploys".to_owned(), fault: db::ColumnFault::Missing })?;
        db.batch(vec![Statement::prepare(MARK).bind(key.clone()), Statement::prepare(UNPUBLISH).bind(key.clone())]).await?;
        let blobs = self.ports.blob();
        let (mut removed, mut bytes) = (0_u64, 0_u64);
        for listed in blobs.list(&tenant).await? {
            if let Deleted::Removed(size) = blobs.delete(&tenant, listed.hash).await? {
                removed += 1;
                bytes = bytes.saturating_add(size.count());
            }
        }
        self.erase(&site.handle.to_string(), id, at).await?;
        Ok(Receipt {
            site: id,
            account: site.account,
            handle: site.handle,
            removed: at,
            deploys: counts.get("deploys")?,
            blobs: removed,
            bytes: Bytes(bytes),
            builds: counts.get("builds")?,
            credentials: counts.get("credentials")?,
        })
    }

    /// Deletes every row of a site, children first, and leaves its handle as a tombstone.
    async fn erase(&self, handle: &str, id: Id<SiteKind>, at: Timestamp) -> Result<(), Error<P>> {
        const ERASE: [&str; 8] = [
            "DELETE FROM event WHERE site_id = ?",
            "DELETE FROM site_live WHERE site_id = ?",
            "DELETE FROM publication WHERE site_id = ?",
            "DELETE FROM job WHERE site_id = ?",
            "DELETE FROM credential WHERE site_id = ?",
            "DELETE FROM deploy WHERE site_id = ?",
            "DELETE FROM source WHERE site_id = ?",
            "DELETE FROM site WHERE id = ?",
        ];
        const TOMBSTONE: &str = "INSERT INTO tombstone (handle, site_id, removed_at) VALUES (?, ?, ?) ON CONFLICT (handle) DO NOTHING";
        let mut batch: Vec<Statement> = ERASE.into_iter().map(|sql| Statement::prepare(sql).bind(id.to_string())).collect();
        batch.push(Statement::prepare(TOMBSTONE).bind(handle).bind(id.to_string()).bind(at.as_millisecond()));
        self.ports.db().batch(batch).await?;
        Ok(())
    }
}
