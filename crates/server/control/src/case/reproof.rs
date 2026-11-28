//! A proved custom domain is looked at again (decision 0151): the TXT record is read on a
//! schedule, a name whose record no longer carries its token stops resolving at once, and is
//! freed for any site after a grace.

use db::{Db, Statement};
use jiff::Timestamp;
use sea_query::{Expr, ExprTrait, Order, Query};

use crate::domain::{Checking, Due, Proof, Verdict};
use crate::fields::Fields;
use crate::id::{Id, Site as SiteKind};
use crate::table;
use crate::{Control, DomainName, Error, Event, Ports, Refusal};

impl<P: Ports> Control<P> {
    /// The proved names whose proof was last seen `Policy::reproof` before `at` or longer ago,
    /// the oldest first, at most `limit`. A lapsed name is among them: its record may be back.
    pub async fn due_domains(&self, at: Timestamp, limit: u64) -> Result<Vec<Due>, Error<P>> {
        let ago = at.as_millisecond().saturating_sub(Self::millis(self.policy.reproof));
        let known = Query::select()
            .columns(Proof::COLUMNS)
            .from(table::Proof::Table)
            .and_where(Expr::col(table::Proof::CheckedAt).lte(ago))
            .order_by(table::Proof::CheckedAt, Order::Asc)
            .limit(limit)
            .to_owned();
        let mut due = Vec::new();
        for row in self.ports.db().rows(known.into()).await? {
            let proof = Proof::read(&row)?;
            due.push(Due { site: proof.site, name: proof.name, token: proof.token });
        }
        // a name proved before the token was kept has no proof row: its token is unknown
        let names = Query::select().column(table::Proof::Name).from(table::Proof::Table).to_owned();
        let legacy = Query::select()
            .columns([table::Domain::Name, table::Domain::SiteId])
            .from(table::Domain::Table)
            .and_where(Expr::col(table::Domain::ProvenAt).lte(ago))
            .and_where(Expr::col(table::Domain::Name).not_in_subquery(names))
            .order_by(table::Domain::ProvenAt, Order::Asc)
            .limit(limit)
            .to_owned();
        for row in self.ports.db().rows(legacy.into()).await? {
            let fields = Fields(&row);
            let name =
                fields.text(table::Domain::Name)?.parse().map_err(|source| crate::Unreadable::Domain { source })?;
            due.push(Due { site: fields.id(table::Domain::SiteId)?, name, token: None });
        }
        due.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(due)
    }

    /// Records what a new read of the proof record of a name found. A record that carries the
    /// token keeps the name serving; one that does not makes it lapse, and it stops resolving at
    /// once. A lapsed name whose record carries the token again serves again. A name proved
    /// before the token was kept cannot be checked, so it lapses: its owner proves it again.
    ///
    /// A read that failed is not a result: the caller records nothing, and the name stays as it was.
    pub async fn recheck_domain(&self, checking: Checking) -> Result<Verdict, Error<P>> {
        let Checking { site, name, found, at } = checking;
        let serving = self.proven(&name).await?.filter(|held| held.site == site);
        let Some(serving) = serving else { return Err(Refusal::NoDomain { site, name }.into()) };
        let before = self.proof_of(&name).await?;
        let token = before.as_ref().and_then(|proof| proof.token);
        let lapsed = before.as_ref().and_then(|proof| proof.lapsed);
        let carried = token.is_some_and(|token| found.has(&token));
        let (standing, lapsed) = match (carried, lapsed) {
            (true, None) => (Verdict::Held, None),
            (true, Some(_)) => (Verdict::Restored, None),
            (false, Some(since)) => (Verdict::Lapsed { since }, Some(since)),
            (false, None) => (Verdict::Lapsed { since: at }, Some(at)),
        };
        let proof = Proof { site, name: serving.name.clone(), token, checked: at, lapsed };
        let mut batch: Vec<Statement> = vec![proof.replace()?];
        let name = serving.name;
        match standing {
            Verdict::Lapsed { .. } if before.as_ref().is_none_or(|proof| proof.lapsed.is_none()) => {
                batch.push(Event::DomainLapsed { name }.insert(site, at)?);
            }
            Verdict::Restored => batch.push(Event::DomainRestored { name }.insert(site, at)?),
            _ => {}
        }
        self.ports.db().batch(batch).await?;
        Ok(standing)
    }

    /// Frees every name that has been lapsed for [`Policy::lapse`](crate::Policy) before `at`:
    /// the proof and the name go, so any site may claim it. Returns how many were freed.
    pub async fn free_lapsed(&self, at: Timestamp) -> Result<u64, Error<P>> {
        let ago = at.as_millisecond().saturating_sub(Self::millis(self.policy.lapse));
        let select = Query::select()
            .columns(Proof::COLUMNS)
            .from(table::Proof::Table)
            .and_where(Expr::col(table::Proof::LapsedAt).lte(ago))
            .order_by(table::Proof::Name, Order::Asc)
            .to_owned();
        let mut freed = 0;
        for row in self.ports.db().rows(select.into()).await? {
            let proof = Proof::read(&row)?;
            self.vacate(&proof, at).await?;
            freed += 1;
        }
        Ok(freed)
    }

    /// Frees `name` if it has been lapsed for the grace: a claim or a proof of it by another site
    /// is then made against a free name.
    pub(crate) async fn free_name(&self, name: &DomainName, at: Timestamp) -> Result<(), Error<P>> {
        let ago = at.as_millisecond().saturating_sub(Self::millis(self.policy.lapse));
        if let Some(proof) =
            self.proof_of(name).await?.filter(|proof| proof.lapsed.is_some_and(|since| since.as_millisecond() <= ago))
        {
            self.vacate(&proof, at).await?;
        }
        Ok(())
    }

    /// The statements that remove a lapsed name, with the event that says so.
    async fn vacate(&self, proof: &Proof, at: Timestamp) -> Result<(), Error<P>> {
        let batch = vec![
            Self::erase_name(table::Domain::Table, table::Domain::Name, &proof.name),
            Self::erase_name(table::Proof::Table, table::Proof::Name, &proof.name),
            Event::DomainRemoved { name: proof.name.clone() }.insert(proof.site, at)?,
        ];
        self.ports.db().batch(batch).await?;
        Ok(())
    }

    /// The statement that deletes the row of `table` whose `column` is `name`.
    pub(crate) fn erase_name<T: sea_query::IntoTableRef, C: sea_query::IntoColumnRef>(
        table: T,
        column: C,
        name: &DomainName,
    ) -> Statement {
        Query::delete().from_table(table).and_where(Expr::col(column).eq(name.as_str())).to_owned().into()
    }

    /// The proof of `name`, if the control plane kept one.
    pub(crate) async fn proof_of(&self, name: &DomainName) -> Result<Option<Proof>, Error<P>> {
        let select = Query::select()
            .columns(Proof::COLUMNS)
            .from(table::Proof::Table)
            .and_where(Expr::col(table::Proof::Name).eq(name.as_str()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| Proof::read(&row)).transpose()?)
    }

    /// The proofs of `site` that lapsed and can still be proved again.
    pub(crate) async fn lapsed(&self, site: Id<SiteKind>) -> Result<Vec<Proof>, Error<P>> {
        let select = Query::select()
            .columns(Proof::COLUMNS)
            .from(table::Proof::Table)
            .and_where(Expr::col(table::Proof::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Proof::LapsedAt).is_not_null())
            .order_by(table::Proof::Name, Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(Proof::read).collect::<Result<_, _>>()?)
    }

    /// `time` in milliseconds, saturating.
    fn millis(time: std::time::Duration) -> i64 {
        i64::try_from(time.as_millis()).unwrap_or(i64::MAX)
    }
}
