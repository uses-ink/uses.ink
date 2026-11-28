//! A removal in steps (decision 0090): what a step reports and what a removal keeps between steps.

mod migration;

use db::{Row, Statement};
use jiff::Timestamp;
use model::Bytes;
use sea_query::{Expr, ExprTrait, Query};

use crate::fields::Fields;
use crate::id::{Account, Id, Site};
use crate::{Handle, Receipt, Unreadable, table};

pub use migration::Erasing;

/// How far a removal has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Progress {
    /// The site being removed.
    pub site: Id<Site>,
    /// Blobs deleted so far, over every step.
    pub blobs: u64,
    /// The bytes they held.
    pub bytes: Bytes,
}

/// What one step of a removal did.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Removal {
    /// The step deleted its share of the blobs and more are left: call again.
    More(Progress),
    /// The last blobs and then the rows are gone.
    Done(Receipt),
}

/// A removal that has begun: the counts taken when it did, and the blobs deleted since.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Begun {
    pub(crate) site: Id<Site>,
    pub(crate) account: Id<Account>,
    pub(crate) handle: Handle,
    pub(crate) deploys: u64,
    pub(crate) builds: u64,
    pub(crate) credentials: u64,
    pub(crate) blobs: u64,
    pub(crate) bytes: u64,
}

impl Begun {
    /// The columns, in the order [`Begun::read`] reads them.
    pub(crate) const COLUMNS: [table::Removal; 8] = [
        table::Removal::SiteId,
        table::Removal::AccountId,
        table::Removal::Handle,
        table::Removal::Deploys,
        table::Removal::Builds,
        table::Removal::Credentials,
        table::Removal::Blobs,
        table::Removal::Bytes,
    ];

    /// A removal from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            site: fields.id(table::Removal::SiteId)?,
            account: fields.id(table::Removal::AccountId)?,
            handle: Handle::stored(row, table::Removal::Handle)?,
            deploys: fields.count(table::Removal::Deploys)?,
            builds: fields.count(table::Removal::Builds)?,
            credentials: fields.count(table::Removal::Credentials)?,
            blobs: fields.count(table::Removal::Blobs)?,
            bytes: fields.count(table::Removal::Bytes)?,
        })
    }

    /// The statement that records this removal, and leaves one that is there already as it is.
    pub(crate) fn insert(&self) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Removal::Table)
            .columns(Self::COLUMNS)
            .values([
                Expr::val(self.site.to_string()),
                Expr::val(self.account.to_string()),
                Expr::val(self.handle.as_str()),
                Expr::val(Self::column(self.deploys)),
                Expr::val(Self::column(self.builds)),
                Expr::val(Self::column(self.credentials)),
                Expr::val(Self::column(self.blobs)),
                Expr::val(Self::column(self.bytes)),
            ])?
            .on_conflict(sea_query::OnConflict::column(table::Removal::SiteId).do_nothing().to_owned())
            .to_owned()
            .into())
    }

    /// A count as the column that holds it.
    fn column(count: u64) -> i64 {
        i64::try_from(count).unwrap_or(i64::MAX)
    }

    /// The statement that adds what a step deleted to the totals.
    pub(crate) fn add(site: Id<Site>, blobs: u64, bytes: u64) -> Statement {
        Query::update()
            .table(table::Removal::Table)
            .value(table::Removal::Blobs, Expr::col(table::Removal::Blobs).add(Self::column(blobs)))
            .value(table::Removal::Bytes, Expr::col(table::Removal::Bytes).add(Self::column(bytes)))
            .and_where(Expr::col(table::Removal::SiteId).eq(site.to_string()))
            .to_owned()
            .into()
    }

    /// The receipt of this removal, finished at `at`, with what the last step deleted added.
    pub(crate) fn receipt(self, last: (u64, u64), at: Timestamp) -> Receipt {
        Receipt {
            site: self.site,
            account: self.account,
            handle: self.handle,
            removed: at,
            deploys: self.deploys,
            blobs: self.blobs.saturating_add(last.0),
            bytes: Bytes(self.bytes.saturating_add(last.1)),
            builds: self.builds,
            credentials: self.credentials,
        }
    }
}
