//! A repository a site proved it controls: the claim, its challenge and the sealed secret of its
//! webhook (decision 0160).

mod migration;

use authority_forge::{Challenge, Repo};
use db::{Row, Statement};
use jiff::Timestamp;
use sea_query::{Expr, Query};
use secret::Plain;

use crate::fields::Fields;
use crate::id::{Credential, Id, Site};
use crate::{Unreadable, table};

pub use migration::Bound;

/// A repository a site asked to prove, and whether it has.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RepoClaim {
    /// The site that asked.
    pub site: Id<Site>,
    /// The repository.
    pub repo: Repo,
    /// The line to commit to `.well-known/uses-ink` on the repository's default branch.
    pub challenge: Challenge,
    /// The credential that holds the webhook secret, sealed.
    pub secret: Id<Credential>,
    /// When it was asked.
    pub created: Timestamp,
    /// When it was proved, or none while it is only asked.
    pub proven: Option<Timestamp>,
}

impl RepoClaim {
    /// The columns, in the order [`RepoClaim::read`] reads them.
    pub(crate) const COLUMNS: [table::Repo; 6] = [
        table::Repo::Repo,
        table::Repo::SiteId,
        table::Repo::Challenge,
        table::Repo::SecretId,
        table::Repo::CreatedAt,
        table::Repo::ProvenAt,
    ];

    /// Whether the repository is proved: pushes to it build the site.
    pub const fn bound(&self) -> bool {
        self.proven.is_some()
    }

    /// A claim from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let repo = fields.text(table::Repo::Repo)?.parse().map_err(|source| Unreadable::Repo { source })?;
        let challenge =
            fields.text(table::Repo::Challenge)?.parse().map_err(|source| Unreadable::Challenge { source })?;
        Ok(Self {
            site: fields.id(table::Repo::SiteId)?,
            repo,
            challenge,
            secret: fields.id(table::Repo::SecretId)?,
            created: fields.time(table::Repo::CreatedAt)?,
            proven: fields.when(table::Repo::ProvenAt)?,
        })
    }

    /// The statement that records this claim.
    pub(crate) fn insert(&self) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Repo::Table)
            .columns(Self::COLUMNS)
            .values([
                Expr::val(self.repo.to_string()),
                Expr::val(self.site.to_string()),
                Expr::val(self.challenge.to_string()),
                Expr::val(self.secret.to_string()),
                Expr::val(self.created.as_millisecond()),
                Expr::val(self.proven.map(Timestamp::as_millisecond)),
            ])?
            .to_owned()
            .into())
    }
}

/// A claim to record: the site, the repository, the challenge and secret the caller drew.
#[derive(Debug)]
pub struct NewRepo {
    /// The site that asks.
    pub site: Id<Site>,
    /// The repository.
    pub repo: Repo,
    /// The challenge, drawn by the caller.
    pub challenge: Challenge,
    /// The webhook secret, drawn by the caller. It is sealed, and shown to the person once by the
    /// caller; it is zeroed when this is dropped.
    pub secret: Plain,
    /// The id of the credential that holds the secret, drawn by the caller.
    pub credential: Id<Credential>,
    /// When it is asked.
    pub at: Timestamp,
}
