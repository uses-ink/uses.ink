//! A handle's hold: what keeps a name from being claimed after its site left it (decision 0087).

mod migration;

use std::fmt;
use std::time::Duration;

use db::{Row, Statement};
use jiff::Timestamp;
use sea_query::{Expr, ExprTrait, OnConflict, Query};
use serde::Serialize;

use crate::fields::Fields;
use crate::id::{Id, Site};
use crate::{Handle, Unreadable, table};

pub use migration::Held;

/// Why a handle is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Why {
    /// Its site was removed: held for a time.
    Removed,
    /// Its site took another handle: held for a time.
    Renamed,
    /// Its owner was verified: held for good, and never given to anyone else.
    Verified,
}

impl Why {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 3] =
        [("removed", Self::Removed), ("renamed", Self::Renamed), ("verified", Self::Verified)];

    /// The text this reason is stored as.
    pub fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("removed", |(name, _)| name)
    }
}

impl fmt::Display for Why {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// The hold on one handle.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Hold {
    /// The handle held.
    pub handle: Handle,
    /// The site that left it, or that was verified under it.
    pub site: Id<Site>,
    /// Why it is held.
    pub why: Why,
    /// When the hold ends. `None` is for good.
    pub until: Option<Timestamp>,
    /// When the hold began.
    pub at: Timestamp,
}

impl Hold {
    /// How long a hold lasts unless the operator says (twelve months, `docs/hosted.md` HQ11).
    pub const DEFAULT: Duration = Duration::from_hours(365 * 24);

    /// The columns, in the order [`Hold::read`] reads them.
    pub(crate) const COLUMNS: [table::Hold; 5] =
        [table::Hold::Handle, table::Hold::SiteId, table::Hold::Why, table::Hold::Until, table::Hold::At];

    /// A hold of `handle` for `why` that began at `at` and does not end.
    pub fn new(handle: Handle, site: Id<Site>, why: Why, at: Timestamp) -> Self {
        Self { handle, site, why, until: None, at }
    }

    /// The same hold, ending `hold` after it began.
    #[must_use]
    pub fn lasting(mut self, hold: Duration) -> Self {
        let millis = i64::try_from(hold.as_millis()).unwrap_or(i64::MAX);
        let end = self.at.as_millisecond().saturating_add(millis);
        self.until = Some(Timestamp::from_millisecond(end).unwrap_or(Timestamp::MAX));
        self
    }

    /// Whether the hold has ended at `at`. A hold for good never does.
    pub fn is_over(&self, at: Timestamp) -> bool {
        self.until.is_some_and(|until| until <= at)
    }

    /// A hold from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            handle: Handle::stored(row, table::Hold::Handle)?,
            site: fields.id(table::Hold::SiteId)?,
            why: fields.name(table::Hold::Why, &Why::NAMES)?,
            until: fields.when(table::Hold::Until)?,
            at: fields.time(table::Hold::At)?,
        })
    }

    /// The statement that records this hold, and leaves one that is there already as it is.
    pub(crate) fn insert(&self) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Hold::Table)
            .columns(Self::COLUMNS)
            .values([
                Expr::val(self.handle.as_str()),
                Expr::val(self.site.to_string()),
                Expr::val(self.why.name()),
                Expr::val(self.until.map(Timestamp::as_millisecond)),
                Expr::val(self.at.as_millisecond()),
            ])?
            .on_conflict(OnConflict::column(table::Hold::Handle).do_nothing().to_owned())
            .to_owned()
            .into())
    }

    /// The statement that records this hold only when no site has the handle any more: the
    /// statement before it moved the site off it.
    pub(crate) fn vacated(&self) -> Result<Statement, sea_query::error::Error> {
        let held = Query::select()
            .column(table::Site::Id)
            .from(table::Site::Table)
            .and_where(Expr::col(table::Site::Handle).eq(self.handle.as_str()))
            .to_owned();
        let mut select = Query::select();
        select
            .exprs([
                Expr::val(self.handle.as_str()),
                Expr::val(self.site.to_string()),
                Expr::val(self.why.name()),
                Expr::val(self.until.map(Timestamp::as_millisecond)),
                Expr::val(self.at.as_millisecond()),
            ])
            .and_where(Expr::not_exists(held));
        let mut insert = Query::insert();
        insert
            .into_table(table::Hold::Table)
            .columns(Self::COLUMNS)
            .select_from(select)?
            .on_conflict(OnConflict::column(table::Hold::Handle).do_nothing().to_owned());
        Ok(insert.into())
    }
}
