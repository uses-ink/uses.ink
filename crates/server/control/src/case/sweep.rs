//! Keeping a site's deploys to the limit, and the files only the dropped ones used (decision
//! 0088). A blob is removed only when a dropped deploy named it and no kept deploy does, so
//! nothing a push has uploaded and not yet published is touched.

use std::collections::BTreeSet;

use blob::{Blob, Chunks, Deleted};
use db::{Db, Statement};
use model::{Bytes, DeployId, Hash, Manifest};
use sea_query::{Expr, ExprTrait, IntoColumnRef, IntoTableRef, Order, Query};

use crate::fields::Fields;
use crate::id::{Id, Site as SiteKind};
use crate::listed::Listed;
use crate::table;
use crate::{Control, Error, Ports};

/// How many ids one statement names: under the smallest bound-parameter limit of a database.
const CHUNK: usize = 40;

/// What a sweep removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Swept {
    /// How many deploys were dropped.
    pub deploys: u64,
    /// How many previews ended and were removed.
    pub previews: u64,
    /// How many blobs were deleted.
    pub blobs: u64,
    /// The bytes those blobs held.
    pub bytes: Bytes,
}

impl<P: Ports> Control<P> {
    /// Drops the deploys of `site` beyond the limit: the live deploy, the one it replaced and
    /// the newest of the rest are kept, up to [`Limits::deploys`](crate::Limits). The files
    /// only a dropped deploy served go with it, bytes before rows (decision 0022).
    pub async fn sweep(&self, site: Id<SiteKind>) -> Result<Swept, Error<P>> {
        self.trim(site, None).await
    }

    /// A sweep as it would be once `incoming`, a deploy not recorded yet, is: the deploy counts
    /// as the newest and its blobs are never removed.
    pub(crate) async fn trim(&self, site: Id<SiteKind>, incoming: Option<&Manifest>) -> Result<Swept, Error<P>> {
        let site = self.site(site).await?;
        let mut ids = self.deploy_ids(site.id).await?;
        let mut protect = BTreeSet::new();
        if let Some(manifest) = incoming {
            let id = manifest.id();
            ids.retain(|known| *known != id);
            ids.insert(0, id);
            protect.insert(id.hash());
            protect.extend(manifest.entries().iter().map(model::Served::blob));
        }
        let limit = usize::try_from(self.policy.limits.deploys).unwrap_or(usize::MAX);
        if ids.len() <= limit {
            return Ok(Swept::default());
        }
        let mut kept: BTreeSet<DeployId> = ids.iter().copied().take(limit).collect();
        if let Some(live) = self.live(site.id).await? {
            kept.insert(live.deploy);
            // an incoming deploy that publishes takes the pointer, and the live deploy becomes
            // its previous: the one before that is no longer needed
            if incoming.is_none() || live.paused {
                kept.extend(live.previous);
            }
        }
        let dropped: Vec<DeployId> = ids.into_iter().filter(|id| !kept.contains(id)).collect();
        self.drop_deploys(site.id, &kept, (&dropped, protect)).await
    }

    /// Removes `dropped` deploys of `site` and the blobs they named that no `kept` deploy and no
    /// blob of `protect` reaches. The second of `dropped` is what to protect.
    pub(crate) async fn drop_deploys(
        &self,
        site: Id<SiteKind>,
        kept: &BTreeSet<DeployId>,
        dropped: (&[DeployId], BTreeSet<Hash>),
    ) -> Result<Swept, Error<P>> {
        let (dropped, mut protected) = dropped;
        let tenant = Self::tenant(site)?;
        for deploy in kept.iter().copied().chain(self.preview_ids(site, &[]).await?) {
            protected.extend(self.reach(site, deploy).await?);
        }
        let mut goners = BTreeSet::new();
        for deploy in dropped {
            goners.extend(self.reach(site, *deploy).await?);
        }
        let mut swept = Swept { deploys: dropped.len() as u64, ..Swept::default() };
        for hash in goners.difference(&protected) {
            if let Deleted::Removed(size) = self.ports.blob().delete(&tenant, *hash).await? {
                swept.blobs += 1;
                swept.bytes = Bytes(swept.bytes.count().saturating_add(size.count()));
            }
        }
        let key = site.to_string();
        let mut batch: Vec<Statement> = Vec::new();
        for chunk in dropped.chunks(CHUNK) {
            let names: Vec<String> = chunk.iter().map(ToString::to_string).collect();
            let publications = (table::Publication::SiteId, key.as_str());
            batch.push(Self::forget(table::Publication::Table, publications, (table::Publication::DeployId, &names)));
            let deploys = (table::Deploy::SiteId, key.as_str());
            batch.push(Self::forget(table::Deploy::Table, deploys, (table::Deploy::Id, &names)));
        }
        self.ports.db().batch(batch).await?;
        Ok(swept)
    }

    /// The statement that deletes the rows of `table` of the site `key` whose `column` is one of
    /// `names`: `site` and `column` are the table's columns for them.
    fn forget<T: IntoTableRef, S: IntoColumnRef, C: IntoColumnRef>(
        table: T,
        site: (S, &str),
        column: (C, &[String]),
    ) -> Statement {
        Query::delete()
            .from_table(table)
            .and_where(Expr::col(site.0).eq(site.1))
            .and_where(Expr::col(column.0).is_in(column.1.to_vec()))
            .to_owned()
            .into()
    }

    /// Removes the blobs `manifest` names that no recorded deploy of `site` reaches: what a
    /// refused deploy brought and nothing else uses.
    pub(crate) async fn discard(&self, site: Id<SiteKind>, manifest: &Manifest) -> Result<(), Error<P>> {
        let tenant = Self::tenant(site)?;
        let mut held = BTreeSet::new();
        for deploy in self.deploy_ids(site).await?.into_iter().chain(self.preview_ids(site, &[]).await?) {
            held.extend(self.reach(site, deploy).await?);
        }
        for entry in manifest.entries() {
            if !held.contains(&entry.blob()) {
                self.ports.blob().delete(&tenant, entry.blob()).await?;
            }
        }
        Ok(())
    }

    /// The ids of `site`'s deploys, newest first.
    pub(crate) async fn deploy_ids(&self, site: Id<SiteKind>) -> Result<Vec<DeployId>, Error<P>> {
        let select = Query::select()
            .column(table::Deploy::Id)
            .from(table::Deploy::Table)
            .and_where(Expr::col(table::Deploy::SiteId).eq(site.to_string()))
            .order_by(table::Deploy::CreatedAt, Order::Desc)
            .order_by(table::Row::Rowid, Order::Desc)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(|row| Fields(row).deploy(table::Deploy::Id)).collect::<Result<_, _>>()?)
    }

    /// The manifest of a deploy, and every blob it names. A manifest the store no longer holds
    /// reaches nothing; one that is not canonical stops the sweep.
    pub(crate) async fn reach(&self, site: Id<SiteKind>, deploy: DeployId) -> Result<BTreeSet<Hash>, Error<P>> {
        let tenant = Self::tenant(site)?;
        let mut reached = BTreeSet::new();
        let Some(stored) = self.ports.blob().get(&tenant, deploy.hash()).await? else { return Ok(reached) };
        let bytes = stored.all().await?;
        let listed = Listed::try_from(bytes.as_slice()).map_err(Error::Manifest)?;
        reached.insert(deploy.hash());
        reached.extend(listed.entries().iter().map(|(_, blob)| *blob));
        Ok(reached)
    }
}
