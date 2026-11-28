//! Accounts and sites.

use db::{Constraint, Db, Statement};
use sea_query::{Expr, ExprTrait, Query};

use crate::account::{self, Account};
use crate::fields::Fields;
use crate::id::{Id, Site as SiteKind};
use crate::site::{self, Site, Stored};
use crate::{Control, Error, Event, Handle, Ports, Refusal, table};

impl<P: Ports> Control<P> {
    /// Adds an account.
    pub async fn add_account(&self, new: account::New) -> Result<Account, Error<P>> {
        let insert = Query::insert()
            .into_table(table::Account::Table)
            .columns([table::Account::Id, table::Account::CreatedAt])
            .values([Expr::val(new.id.to_string()), Expr::val(new.at.as_millisecond())])?
            .to_owned();
        match self.ports.db().run(insert.into()).await {
            Ok(_) => Ok(Account { id: new.id, created: new.at }),
            Err(error) if error.constraint() == Some(Constraint::Unique) => {
                Err(Refusal::Exists { id: new.id.to_string() }.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Adds a site: claims its handle and records its source, in one batch with the event that
    /// says so. A handle that is reserved, on the brand list, taken, or held after a site left
    /// it is refused ([`Control::claimable`]); a hold that is over is released by the claim.
    pub async fn add_site(&self, new: site::New) -> Result<Site, Error<P>> {
        let stored = Stored::try_from(&new.source)?;
        self.claimable(&new.handle, new.at).await?;
        self.site_room(new.account).await?;
        let db = self.ports.db();
        let added = Event::SiteAdded { handle: new.handle.clone() };
        let mut batch = Self::release(&new.handle, new.at);
        batch.extend([Self::site_row(&new)?, Self::source_row(new.id, stored)?, added.insert(new.id, new.at)?]);
        match db.batch(batch).await {
            Ok(_) => Ok(Site {
                id: new.id,
                account: new.account,
                handle: new.handle,
                state: site::State::Active,
                created: new.at,
                source: new.source,
            }),
            Err(error) => match error.constraint() {
                Some(Constraint::Unique) => Err(self.taken(new.id, new.handle).await),
                Some(Constraint::Reference) => Err(Refusal::NoAccount { account: new.account }.into()),
                _ => Err(error.into()),
            },
        }
    }

    /// Why a unique key failed on a new site: the handle, or an id that exists.
    async fn taken(&self, id: Id<SiteKind>, handle: Handle) -> Error<P> {
        let held = Query::select()
            .column(table::Site::Id)
            .from(table::Site::Table)
            .and_where(Expr::col(table::Site::Handle).eq(handle.as_str()))
            .to_owned();
        match self.ports.db().first(held.into()).await {
            Ok(Some(_)) => Refusal::Taken { handle }.into(),
            Ok(None) => Refusal::Exists { id: id.to_string() }.into(),
            Err(error) => error.into(),
        }
    }

    /// The site with this id, whatever its state.
    pub async fn site(&self, id: Id<SiteKind>) -> Result<Site, Error<P>> {
        let select = Query::select()
            .columns([
                (table::Site::Table, table::Site::Id),
                (table::Site::Table, table::Site::AccountId),
                (table::Site::Table, table::Site::Handle),
                (table::Site::Table, table::Site::State),
                (table::Site::Table, table::Site::CreatedAt),
            ])
            .columns([
                (table::Source::Table, table::Source::Kind),
                (table::Source::Table, table::Source::Locator),
                (table::Source::Table, table::Source::Revision),
            ])
            .from(table::Site::Table)
            .inner_join(
                table::Source::Table,
                Expr::col((table::Source::Table, table::Source::SiteId)).equals((table::Site::Table, table::Site::Id)),
            )
            .and_where(Expr::col((table::Site::Table, table::Site::Id)).eq(id.to_string()))
            .to_owned();
        match self.ports.db().first(select.into()).await? {
            Some(row) => Ok(Site::read(&row)?),
            None => Err(Refusal::NoSite { site: id }.into()),
        }
    }

    /// The site that owns this handle, if one does: what the edge asks first.
    pub async fn by_handle(&self, handle: &Handle) -> Result<Option<Site>, Error<P>> {
        let select = Query::select()
            .column(table::Site::Id)
            .from(table::Site::Table)
            .and_where(Expr::col(table::Site::Handle).eq(handle.as_str()))
            .to_owned();
        match self.ports.db().first(select.into()).await? {
            Some(row) => Ok(Some(self.site(Fields(&row).id(table::Site::Id)?).await?)),
            None => Ok(None),
        }
    }

    /// The site, which must take changes: refused if it is being removed.
    pub(crate) async fn active(&self, id: Id<SiteKind>) -> Result<Site, Error<P>> {
        let site = self.site(id).await?;
        match site.state {
            site::State::Active => Ok(site),
            _ => Err(Refusal::Removing { site: id }.into()),
        }
    }

    /// The statement that adds the site's row.
    fn site_row(new: &site::New) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Site::Table)
            .columns([
                table::Site::Id,
                table::Site::AccountId,
                table::Site::Handle,
                table::Site::State,
                table::Site::CreatedAt,
            ])
            .values([
                Expr::val(new.id.to_string()),
                Expr::val(new.account.to_string()),
                Expr::val(new.handle.as_str()),
                Expr::val(site::State::Active.name()),
                Expr::val(new.at.as_millisecond()),
            ])?
            .to_owned()
            .into())
    }

    /// The statement that adds the row of the site's source.
    fn source_row(id: Id<SiteKind>, stored: Stored) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Source::Table)
            .columns([table::Source::SiteId, table::Source::Kind, table::Source::Locator, table::Source::Revision])
            .values([
                Expr::val(id.to_string()),
                Expr::val(Stored::name(stored.kind)),
                Expr::val(stored.locator),
                Expr::val(stored.revision),
            ])?
            .to_owned()
            .into())
    }

    /// The tenant a site's blobs are under.
    pub(crate) fn tenant(site: Id<SiteKind>) -> Result<blob::Tenant, Refusal> {
        blob::Tenant::try_from(site).map_err(Refusal::Tenant)
    }
}
