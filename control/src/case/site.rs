//! Accounts and sites.

use db::{Constraint, Db, Statement};

use crate::account::{self, Account};
use crate::fields::Fields;
use crate::id::{Id, Site as SiteKind};
use crate::site::{self, Site, Stored};
use crate::{Control, Error, Event, Handle, Ports, Refusal};

impl<P: Ports> Control<P> {
    /// Adds an account.
    pub async fn add_account(&self, new: account::New) -> Result<Account, Error<P>> {
        const INSERT: &str = "INSERT INTO account (id, created_at) VALUES (?, ?)";
        let insert = Statement::prepare(INSERT).bind(new.id.to_string()).bind(new.at.as_millisecond());
        match self.ports.db().run(insert).await {
            Ok(_) => Ok(Account { id: new.id, created: new.at }),
            Err(error) if error.constraint() == Some(Constraint::Unique) => {
                Err(Refusal::Exists { id: new.id.to_string() }.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Adds a site: claims its handle and records its source, in one batch with the event that
    /// says so. A handle that is reserved, taken, or left by a removed site is refused.
    pub async fn add_site(&self, new: site::New) -> Result<Site, Error<P>> {
        const INSERT: &str = "INSERT INTO site (id, account_id, handle, state, created_at) VALUES (?, ?, ?, 'active', ?)";
        const SOURCE: &str = "INSERT INTO source (site_id, kind, locator, revision) VALUES (?, ?, ?, ?)";
        const TOMBSTONE: &str = "SELECT 1 AS held FROM tombstone WHERE handle = ?";
        if new.handle.is_reserved() {
            return Err(Refusal::Reserved { handle: new.handle }.into());
        }
        let stored = Stored::try_from(&new.source)?;
        let db = self.ports.db();
        if db.first(Statement::prepare(TOMBSTONE).bind(new.handle.as_str())).await?.is_some() {
            return Err(Refusal::Tombstoned { handle: new.handle }.into());
        }
        let added = Event::SiteAdded { handle: new.handle.clone() };
        let batch = vec![
            Statement::prepare(INSERT)
                .bind(new.id.to_string())
                .bind(new.account.to_string())
                .bind(new.handle.as_str())
                .bind(new.at.as_millisecond()),
            Statement::prepare(SOURCE).bind(new.id.to_string()).bind(Stored::name(stored.kind)).bind(stored.locator).bind(stored.revision),
            added.insert(new.id, new.at).map_err(Error::Log)?,
        ];
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
        const HELD: &str = "SELECT 1 AS held FROM site WHERE handle = ?";
        match self.ports.db().first(Statement::prepare(HELD).bind(handle.as_str())).await {
            Ok(Some(_)) => Refusal::Taken { handle }.into(),
            Ok(None) => Refusal::Exists { id: id.to_string() }.into(),
            Err(error) => error.into(),
        }
    }

    /// The site with this id, whatever its state.
    pub async fn site(&self, id: Id<SiteKind>) -> Result<Site, Error<P>> {
        const SELECT: &str = "SELECT s.id, s.account_id, s.handle, s.state, s.created_at, o.kind, o.locator, o.revision \
            FROM site s JOIN source o ON o.site_id = s.id WHERE s.id = ?";
        match self.ports.db().first(Statement::prepare(SELECT).bind(id.to_string())).await? {
            Some(row) => Ok(Site::read(&row)?),
            None => Err(Refusal::NoSite { site: id }.into()),
        }
    }

    /// The site that owns this handle, if one does: what the edge asks first.
    pub async fn by_handle(&self, handle: &Handle) -> Result<Option<Site>, Error<P>> {
        const SELECT: &str = "SELECT id FROM site WHERE handle = ?";
        match self.ports.db().first(Statement::prepare(SELECT).bind(handle.as_str())).await? {
            Some(row) => Ok(Some(self.site(Fields(&row).id("id")?).await?)),
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

    /// The tenant a site's blobs are under.
    pub(crate) fn tenant(site: Id<SiteKind>) -> Result<blob::Tenant, Refusal> {
        blob::Tenant::try_from(site).map_err(Refusal::Tenant)
    }
}

