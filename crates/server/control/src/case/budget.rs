//! What an account has spent of its budgets, and the refusal when a request would take it over
//! (decision 0088). Counting reads the rows and the blob store; nothing is cached.

use blob::Blob;
use db::Db;
use jiff::Timestamp;
use model::{Bytes, Manifest};
use sea_query::{Asterisk, Expr, ExprTrait, Func, IdenStatic, Query};

use crate::fields::Fields;
use crate::id::{Account as AccountKind, Id};
use crate::table::{self, Computed};
use crate::{Control, Error, Ports, Site, Spent};

/// How long a day of builds is, in milliseconds.
const DAY: i64 = 24 * 60 * 60 * 1000;

impl<P: Ports> Control<P> {
    /// How many bytes the account stores: every blob of every site's tenant, but those queued for
    /// deletion (decision 0150).
    pub async fn stored(&self, account: Id<AccountKind>) -> Result<Bytes, Error<P>> {
        let sites = Query::select()
            .column(table::Site::Id)
            .from(table::Site::Table)
            .and_where(Expr::col(table::Site::AccountId).eq(account.to_string()))
            .to_owned();
        let mut total = 0_u64;
        for row in self.ports.db().rows(sites.into()).await? {
            let site = Fields(&row).id(table::Site::Id)?;
            let tenant = Self::tenant(site)?;
            for listed in self.ports.blob().list(&tenant).await? {
                total = total.saturating_add(listed.size.count());
            }
            // a queued blob is stored until its grace is over, and is not the account's to pay for
            total = total.saturating_sub(self.waiting(site).await?);
        }
        Ok(Bytes(total))
    }

    /// How many builds the account started in the 24 hours before `at`.
    pub async fn count_builds(&self, account: Id<AccountKind>, at: Timestamp) -> Result<u64, Error<P>> {
        let count = Query::select()
            .expr_as(Func::count(Expr::col(Asterisk)), Computed::Count)
            .from(table::Job::Table)
            .inner_join(
                table::Site::Table,
                Expr::col((table::Job::Table, table::Job::SiteId)).equals((table::Site::Table, table::Site::Id)),
            )
            .and_where(Expr::col((table::Site::Table, table::Site::AccountId)).eq(account.to_string()))
            .and_where(Expr::col((table::Job::Table, table::Job::RequestedAt)).gt(at.as_millisecond() - DAY))
            .to_owned();
        self.counted(count).await
    }

    /// How many sites the account has, whatever their state.
    pub async fn count_sites(&self, account: Id<AccountKind>) -> Result<u64, Error<P>> {
        let count = Query::select()
            .expr_as(Func::count(Expr::col(Asterisk)), Computed::Count)
            .from(table::Site::Table)
            .and_where(Expr::col(table::Site::AccountId).eq(account.to_string()))
            .to_owned();
        self.counted(count).await
    }

    pub(crate) async fn counted(&self, count: sea_query::SelectStatement) -> Result<u64, Error<P>> {
        let row = self.ports.db().first(count.into()).await?;
        let row = row
            .ok_or(db::ColumnError { column: Computed::Count.as_str().to_owned(), fault: db::ColumnFault::Missing })?;
        Ok(row.get(Computed::Count)?)
    }

    /// Refuses a new site when the account has as many as it may.
    pub(crate) async fn site_room(&self, account: Id<AccountKind>) -> Result<(), Error<P>> {
        let sites = self.count_sites(account).await?;
        if sites >= self.policy.limits.sites {
            return Err(self.policy.limits.over(Spent::Sites, sites).into());
        }
        Ok(())
    }

    /// Refuses a build when the account started as many in the last day as it may.
    pub(crate) async fn build_room(&self, account: Id<AccountKind>, at: Timestamp) -> Result<(), Error<P>> {
        let builds = self.count_builds(account, at).await?;
        if builds >= self.policy.limits.builds {
            return Err(self.policy.limits.over(Spent::Builds, builds).into());
        }
        Ok(())
    }

    /// Refuses a deploy that leaves the account over its storage, whole: nothing of it is
    /// recorded. The blobs of `manifest` are in the store already, so they are counted. Over the
    /// limit, the site's older deploys are swept first, as they will be once this one is
    /// recorded; still over, the blobs only this deploy brought are removed, so a refused deploy
    /// leaves no bytes.
    pub(crate) async fn store_room(&self, site: &Site, manifest: &Manifest, at: Timestamp) -> Result<(), Error<P>> {
        let limit = self.policy.limits.stored;
        if self.stored(site.account).await? <= limit {
            return Ok(());
        }
        self.trim(site.id, Some(manifest), at).await?;
        let total = self.stored(site.account).await?;
        if total <= limit {
            return Ok(());
        }
        self.discard(site.id, manifest, at).await?;
        Err(self.policy.limits.over(Spent::Stored, total.count()).into())
    }
}
