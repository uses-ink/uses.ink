//! Custom domains (decision 0093): a site asks to prove a name, publishes a token in DNS, and
//! the name then serves the site until the site or the name is removed.

use db::{Constraint, Db, Statement};
use jiff::Timestamp;
use sea_query::{Asterisk, Expr, ExprTrait, Func, Order, Query};

use crate::domain::{Claim, Domains, Found, NewClaim, Proven};
use crate::id::{Id, Site as SiteKind};
use crate::table::{self, Computed};
use crate::{Control, DomainName, Error, Event, Ports, Refusal, Site, SiteState};

/// A proof to check: the claim of `name` by `site`, and what a lookup of its TXT record found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proving {
    /// The site that claimed the name.
    pub site: Id<SiteKind>,
    /// The name.
    pub name: DomainName,
    /// What the authority read from DNS.
    pub found: Found,
    /// When it was read.
    pub at: Timestamp,
}

impl<P: Ports> Control<P> {
    /// Records that `new.site` asks to prove `new.name`, and returns the claim with the token the
    /// TXT record must carry. Asking again replaces the site's earlier token for the name.
    ///
    /// A name the operator serves, a name another site proved, and a site that has as many
    /// domains as the policy allows are refused. A claim serves nothing: only [`Control::prove_domain`]
    /// makes a name serve, and any number of sites may hold a claim of one name until one proves it.
    pub async fn claim_domain(&self, new: NewClaim) -> Result<Claim, Error<P>> {
        let site = self.active(new.site).await?;
        if self.origin.covers(&new.name) {
            return Err(Refusal::DomainOurs { name: new.name }.into());
        }
        match self.proven(&new.name).await? {
            Some(held) if held.site == site.id => {
                return Err(Refusal::Exists { id: new.name.to_string() }.into());
            }
            Some(_) => return Err(Refusal::DomainTaken { name: new.name }.into()),
            None => {}
        }
        let proven = Query::select()
            .expr_as(Func::count(Expr::col(Asterisk)), Computed::Count)
            .from(table::Domain::Table)
            .and_where(Expr::col(table::Domain::SiteId).eq(site.id.to_string()))
            .to_owned();
        let pending = Query::select()
            .expr_as(Func::count(Expr::col(Asterisk)), Computed::Count)
            .from(table::Claim::Table)
            .and_where(Expr::col(table::Claim::SiteId).eq(site.id.to_string()))
            .and_where(Expr::col(table::Claim::Name).ne(new.name.as_str()))
            .to_owned();
        let held = self.counted(proven).await?.saturating_add(self.counted(pending).await?);
        if held >= self.policy.domains {
            return Err(Refusal::DomainRoom { site: site.id, limit: self.policy.domains }.into());
        }
        let claim = Claim { site: site.id, name: new.name, token: new.token, created: new.at };
        let event = Event::DomainClaimed { name: claim.name.clone() }.insert(site.id, new.at)?;
        self.ports.db().batch(vec![claim.replace()?, event]).await?;
        Ok(claim)
    }

    /// Makes the name of a claim serve its site, when the TXT record the authority read carries
    /// the claim's token. The proof binds the name to the site that claimed it: another site's
    /// claim is not proved by this token, and once a site has the name, no other can.
    ///
    /// The name's row is its primary key, so of two sites that prove at once the database lets
    /// one. Every claim of the name is dropped in the same batch. Proving a name the site already
    /// serves answers the row it has.
    pub async fn prove_domain(&self, proving: Proving) -> Result<Proven, Error<P>> {
        let Proving { site, name, found, at } = proving;
        let site = self.active(site).await?;
        if let Some(held) = self.proven(&name).await? {
            return if held.site == site.id { Ok(held) } else { Err(Refusal::DomainTaken { name }.into()) };
        }
        let claim = self
            .claim_of(site.id, &name)
            .await?
            .ok_or_else(|| Refusal::NoClaim { site: site.id, name: name.clone() })?;
        if !found.has(&claim.token) {
            return Err(Refusal::Unproven { name, records: found.records }.into());
        }
        let domain = Proven { site: site.id, name, at };
        let drop = Query::delete()
            .from_table(table::Claim::Table)
            .and_where(Expr::col(table::Claim::Name).eq(domain.name.as_str()))
            .to_owned();
        let event = Event::DomainProven { name: domain.name.clone() }.insert(site.id, at)?;
        match self.ports.db().batch(vec![domain.insert()?, drop.into(), event]).await {
            Ok(_) => Ok(domain),
            Err(error) if error.constraint() == Some(Constraint::Unique) => {
                Err(Refusal::DomainTaken { name: domain.name }.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Stops `site` serving `name`, or drops its claim of it, so the name is free for any site to
    /// claim. A site that is being removed or suspended may still remove its domains.
    pub async fn remove_domain(&self, site: Id<SiteKind>, name: &DomainName, at: Timestamp) -> Result<(), Error<P>> {
        let site = self.site(site).await?;
        let served = self.proven(name).await?.is_some_and(|held| held.site == site.id);
        let asked = self.claim_of(site.id, name).await?.is_some();
        if !served && !asked {
            return Err(Refusal::NoDomain { site: site.id, name: name.clone() }.into());
        }
        let key = site.id.to_string();
        let mut batch: Vec<Statement> = vec![
            Query::delete()
                .from_table(table::Domain::Table)
                .and_where(Expr::col(table::Domain::Name).eq(name.as_str()))
                .and_where(Expr::col(table::Domain::SiteId).eq(key.clone()))
                .to_owned()
                .into(),
            Query::delete()
                .from_table(table::Claim::Table)
                .and_where(Expr::col(table::Claim::Name).eq(name.as_str()))
                .and_where(Expr::col(table::Claim::SiteId).eq(key))
                .to_owned()
                .into(),
        ];
        batch.push(Event::DomainRemoved { name: name.clone() }.insert(site.id, at)?);
        self.ports.db().batch(batch).await?;
        Ok(())
    }

    /// The names `site` serves and the ones it has still to prove, each by name.
    pub async fn domains(&self, site: Id<SiteKind>) -> Result<Domains, Error<P>> {
        self.site(site).await?;
        let proven = Query::select()
            .columns(Proven::COLUMNS)
            .from(table::Domain::Table)
            .and_where(Expr::col(table::Domain::SiteId).eq(site.to_string()))
            .order_by(table::Domain::Name, Order::Asc)
            .to_owned();
        let pending = Query::select()
            .columns(Claim::COLUMNS)
            .from(table::Claim::Table)
            .and_where(Expr::col(table::Claim::SiteId).eq(site.to_string()))
            .order_by(table::Claim::Name, Order::Asc)
            .to_owned();
        let db = self.ports.db();
        Ok(Domains {
            proven: db.rows(proven.into()).await?.iter().map(Proven::read).collect::<Result<_, _>>()?,
            pending: db.rows(pending.into()).await?.iter().map(Claim::read).collect::<Result<_, _>>()?,
        })
    }

    /// The site that serves `name` as a custom domain, when it takes changes: what the edge asks
    /// for a host that is no handle's. A claim that was not proved, and a site being removed, are
    /// no answer.
    pub async fn domained(&self, name: &DomainName) -> Result<Option<Site>, Error<P>> {
        let Some(held) = self.proven(name).await? else { return Ok(None) };
        Ok(Some(self.site(held.site).await?).filter(|site| site.state == SiteState::Active))
    }

    /// The row of a proved name, if there is one.
    async fn proven(&self, name: &DomainName) -> Result<Option<Proven>, Error<P>> {
        let select = Query::select()
            .columns(Proven::COLUMNS)
            .from(table::Domain::Table)
            .and_where(Expr::col(table::Domain::Name).eq(name.as_str()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| Proven::read(&row)).transpose()?)
    }

    /// The claim of `name` by `site`, if there is one.
    async fn claim_of(&self, site: Id<SiteKind>, name: &DomainName) -> Result<Option<Claim>, Error<P>> {
        let select = Query::select()
            .columns(Claim::COLUMNS)
            .from(table::Claim::Table)
            .and_where(Expr::col(table::Claim::Name).eq(name.as_str()))
            .and_where(Expr::col(table::Claim::SiteId).eq(site.to_string()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| Claim::read(&row)).transpose()?)
    }
}
