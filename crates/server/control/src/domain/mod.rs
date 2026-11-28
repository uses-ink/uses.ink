//! A custom domain: the name a site asks to serve, the token its TXT record must carry, and the
//! proof that binds the name to the site (decision 0093).

mod migration;
mod name;
mod token;

use db::{Row, Statement};
use jiff::Timestamp;
use sea_query::{Expr, OnConflict, Query};

use crate::fields::Fields;
use crate::id::{Id, Site};
use crate::{Unreadable, table};

pub use migration::Domained;
pub use name::{DomainName, NameError};
pub use token::{Found, Token, TokenError};

/// A name a site asked to prove and has not. It serves nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Claim {
    /// The site that asked.
    pub site: Id<Site>,
    /// The name.
    pub name: DomainName,
    /// What the TXT record at `_uses-ink.<name>` must say to prove the name is this site's.
    pub token: Token,
    /// When it was asked.
    pub created: Timestamp,
}

impl Claim {
    /// The columns, in the order [`Claim::read`] reads them.
    pub(crate) const COLUMNS: [table::Claim; 4] =
        [table::Claim::Name, table::Claim::SiteId, table::Claim::Token, table::Claim::CreatedAt];

    /// A claim from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let name = fields.text(table::Claim::Name)?.parse().map_err(|source| Unreadable::Domain { source })?;
        let token = fields.text(table::Claim::Token)?.parse().map_err(|source| Unreadable::Token { source })?;
        Ok(Self { site: fields.id(table::Claim::SiteId)?, name, token, created: fields.time(table::Claim::CreatedAt)? })
    }

    /// The statement that records this claim, in place of the site's earlier claim of the name.
    pub(crate) fn replace(&self) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Claim::Table)
            .columns(Self::COLUMNS)
            .values([
                Expr::val(self.name.as_str()),
                Expr::val(self.site.to_string()),
                Expr::val(self.token.to_string()),
                Expr::val(self.created.as_millisecond()),
            ])?
            .on_conflict(
                OnConflict::columns([table::Claim::Name, table::Claim::SiteId])
                    .update_columns([table::Claim::Token, table::Claim::CreatedAt])
                    .to_owned(),
            )
            .to_owned()
            .into())
    }
}

/// A name a site has proved: the host serves the site, whatever handle it has.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Proven {
    /// The site.
    pub site: Id<Site>,
    /// The name.
    pub name: DomainName,
    /// When the proof was seen.
    pub at: Timestamp,
}

impl Proven {
    /// The columns, in the order [`Proven::read`] reads them.
    pub(crate) const COLUMNS: [table::Domain; 3] =
        [table::Domain::Name, table::Domain::SiteId, table::Domain::ProvenAt];

    /// A proved name from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let name = fields.text(table::Domain::Name)?.parse().map_err(|source| Unreadable::Domain { source })?;
        Ok(Self { site: fields.id(table::Domain::SiteId)?, name, at: fields.time(table::Domain::ProvenAt)? })
    }

    /// The statement that records the proof.
    pub(crate) fn insert(&self) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Domain::Table)
            .columns(Self::COLUMNS)
            .values([
                Expr::val(self.name.as_str()),
                Expr::val(self.site.to_string()),
                Expr::val(self.at.as_millisecond()),
            ])?
            .to_owned()
            .into())
    }
}

/// A claim to record: the site, the name and the token the caller drew.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewClaim {
    /// The site that asks.
    pub site: Id<Site>,
    /// The name.
    pub name: DomainName,
    /// The token, drawn by the caller.
    pub token: Token,
    /// When it is asked.
    pub at: Timestamp,
}

/// What a site has of custom domains: the names it proved and the ones it has still to prove.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Domains {
    /// Names that serve the site, by name.
    pub proven: Vec<Proven>,
    /// Names asked and not proved, by name.
    pub pending: Vec<Claim>,
}
