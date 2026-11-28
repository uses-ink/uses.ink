//! Claiming a handle: who may have it, what holds it, verifying its owner and renaming (decision 0087).

use db::Db;
use jiff::Timestamp;
use sea_query::{Expr, ExprTrait, Query};

use crate::fields::Fields;
use crate::id::{Id, Site as SiteKind};
use crate::{Control, Error, Event, Handle, Hold, HoldReason, Ports, Refusal, Site, table};

impl<P: Ports> Control<P> {
    /// The hold on `handle`, if it has one. A tombstone made before holds existed has no row of
    /// its own: it is held from its day for as long as this control plane holds a handle.
    pub async fn held(&self, handle: &Handle) -> Result<Option<Hold>, Error<P>> {
        let hold = Query::select()
            .columns(Hold::COLUMNS)
            .from(table::Hold::Table)
            .and_where(Expr::col(table::Hold::Handle).eq(handle.as_str()))
            .to_owned();
        if let Some(row) = self.ports.db().first(hold.into()).await? {
            return Ok(Some(Hold::read(&row)?));
        }
        let tombstone = Query::select()
            .columns([table::Tombstone::SiteId, table::Tombstone::RemovedAt])
            .from(table::Tombstone::Table)
            .and_where(Expr::col(table::Tombstone::Handle).eq(handle.as_str()))
            .to_owned();
        let Some(row) = self.ports.db().first(tombstone.into()).await? else { return Ok(None) };
        let fields = Fields(&row);
        let site = fields.id(table::Tombstone::SiteId)?;
        let removed = fields.time(table::Tombstone::RemovedAt)?;
        Ok(Some(Hold::new(handle.clone(), site, HoldReason::Removed, removed).lasting(self.policy.hold)))
    }

    /// Whether `at` is a time `handle` may be claimed: no site has it, it is on no list and no
    /// hold on it is still running. Nothing is changed.
    pub(crate) async fn claimable(&self, handle: &Handle, at: Timestamp) -> Result<(), Error<P>> {
        if self.by_handle(handle).await?.is_some() {
            return Err(Refusal::Taken { handle: handle.clone() }.into());
        }
        if let Some(reservation) = handle.reservation()? {
            return Err(Refusal::Reserved { handle: handle.clone(), reservation }.into());
        }
        match self.held(handle).await? {
            Some(hold) if !hold.is_over(at) => {
                Err(Refusal::Held { handle: handle.clone(), why: hold.why, until: hold.until }.into())
            }
            _ => Ok(()),
        }
    }

    /// The statements that free a handle whose hold is over, so a claim can take it: the hold
    /// and the tombstone go. A hold that is not over, and a hold for good, stay.
    pub(crate) fn release(handle: &Handle, at: Timestamp) -> Vec<db::Statement> {
        let hold = Query::delete()
            .from_table(table::Hold::Table)
            .and_where(Expr::col(table::Hold::Handle).eq(handle.as_str()))
            .and_where(Expr::col(table::Hold::Until).lte(at.as_millisecond()))
            .to_owned();
        let tombstone = Query::delete()
            .from_table(table::Tombstone::Table)
            .and_where(Expr::col(table::Tombstone::Handle).eq(handle.as_str()))
            .to_owned();
        vec![hold.into(), tombstone.into()]
    }

    /// Records that the owner of `site` is verified: its handle is held for good once the site
    /// leaves it, and no one else is ever given it. Verifying twice changes nothing.
    pub async fn verify(&self, site: Id<SiteKind>, at: Timestamp) -> Result<Hold, Error<P>> {
        let site = self.active(site).await?;
        if let Some(hold) = self.held(&site.handle).await?.filter(|hold| hold.why == HoldReason::Verified) {
            return Ok(hold);
        }
        let hold = Hold::new(site.handle.clone(), site.id, HoldReason::Verified, at);
        let event = Event::HandleVerified { handle: site.handle };
        self.ports.db().batch(vec![hold.insert()?, event.insert(site.id, at)?]).await?;
        Ok(hold)
    }

    /// Gives a site another handle. The handle it had is held, as if the site had been removed,
    /// so a rename is not a way to free a name for someone else at once.
    pub async fn rename_site(&self, id: Id<SiteKind>, handle: Handle, at: Timestamp) -> Result<Site, Error<P>> {
        let mut site = self.active(id).await?;
        self.claimable(&handle, at).await?;
        let from = site.handle.clone();
        let hold = Hold::new(from.clone(), id, HoldReason::Renamed, at).lasting(self.policy.hold);
        let rename = Query::update()
            .table(table::Site::Table)
            .value(table::Site::Handle, handle.as_str())
            .and_where(Expr::col(table::Site::Id).eq(id.to_string()))
            .and_where(Expr::col(table::Site::Handle).eq(from.as_str()))
            .to_owned();
        let mut batch = Self::release(&handle, at);
        batch.push(rename.into());
        batch.push(hold.vacated()?);
        batch.push(Event::SiteRenamed { from, to: handle.clone() }.insert(id, at)?);
        let outcomes = match self.ports.db().batch(batch).await {
            Ok(outcomes) => outcomes,
            Err(error) if error.constraint() == Some(db::Constraint::Unique) => {
                return Err(Refusal::Taken { handle }.into());
            }
            Err(error) => return Err(error.into()),
        };
        match outcomes.get(2).map(|outcome| outcome.changes) {
            Some(1) => {
                site.handle = handle;
                Ok(site)
            }
            _ => Err(Refusal::Taken { handle }.into()),
        }
    }
}
