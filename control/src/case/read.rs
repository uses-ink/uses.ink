//! Reading across rows: an account's sites, the sites to poll, and a build's log.

use blob::{Blob, Chunks};
use build::BuildId;
use db::Db;
use sea_query::{Expr, ExprTrait, Order, Query};

use crate::fields::Fields;
use crate::id::{Account as AccountKind, Id, Site as SiteKind};
use crate::site::Stored;
use crate::{Control, Error, Ports, Refusal, Site, SiteState, table};

impl<P: Ports> Control<P> {
    /// The ids of the sites whose row says `column` is `value`, oldest first.
    async fn site_ids(&self, filter: Expr) -> Result<Vec<Id<SiteKind>>, Error<P>> {
        let select = Query::select()
            .column((table::Site::Table, table::Site::Id))
            .from(table::Site::Table)
            .inner_join(
                table::Source::Table,
                Expr::col((table::Source::Table, table::Source::SiteId)).equals((table::Site::Table, table::Site::Id)),
            )
            .and_where(filter)
            .order_by((table::Site::Table, table::Site::CreatedAt), Order::Asc)
            .order_by((table::Site::Table, table::Site::Id), Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(|row| Fields(row).id(table::Site::Id)).collect::<Result<_, _>>()?)
    }

    /// Every site of `account`, oldest first, whatever its state.
    pub async fn sites(&self, account: Id<AccountKind>) -> Result<Vec<Site>, Error<P>> {
        let owned = Expr::col((table::Site::Table, table::Site::AccountId)).eq(account.to_string());
        let mut found = Vec::new();
        for id in self.site_ids(owned).await? {
            found.push(self.site(id).await?);
        }
        Ok(found)
    }

    /// Every site that takes changes and whose source is a git repository: the sites a poll
    /// looks at.
    pub async fn watched(&self) -> Result<Vec<Site>, Error<P>> {
        let active = Expr::col((table::Site::Table, table::Site::State)).eq(SiteState::Active.name());
        let git = Expr::col((table::Source::Table, table::Source::Kind)).eq(Stored::name(build::Kind::Git));
        let mut found = Vec::new();
        for id in self.site_ids(active.and(git)).await? {
            found.push(self.site(id).await?);
        }
        Ok(found)
    }

    /// The log of a build of `site`, as the NDJSON that was stored, or `None` while the build has
    /// not written one. A build of another site is no build of this one.
    pub async fn build_log(&self, site: Id<SiteKind>, build: BuildId) -> Result<Option<Vec<u8>>, Error<P>> {
        let job = self.job(build).await?.filter(|job| job.site == site);
        let job = job.ok_or(Refusal::NoBuild { site, build })?;
        let Some(hash) = job.log else { return Ok(None) };
        let tenant = Self::tenant(site)?;
        let stored = self.ports.blob().get(&tenant, hash).await?;
        let stored = stored.ok_or(Refusal::MissingLog { build, blob: hash })?;
        Ok(Some(stored.all().await?))
    }
}
