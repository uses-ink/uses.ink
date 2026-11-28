//! Abuse: what a stranger may report, and the suspension that takes a site down (decision 0089).

mod migration;

use std::fmt;

use db::{Row, Statement};
use jiff::Timestamp;
use sea_query::{Expr, Query};
use serde::{Deserialize, Serialize};

use crate::fields::Fields;
use crate::id::{Id, Report as ReportKind, Site};
use crate::{Handle, Unreadable, table};

pub use migration::Reported;

/// What kind of abuse a report is about, and the reason a site is suspended for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Abuse {
    /// A page that asks for credentials or payment under another's name.
    Phishing,
    /// Software that harms the machine that loads it.
    Malware,
    /// Content that is illegal where the server runs.
    Illegal,
    /// A site that passes itself off as a person or an organisation.
    Impersonation,
    /// Bulk unwanted content.
    Spam,
    /// None of these: the detail says.
    Other,
}

impl Abuse {
    /// The names the columns store.
    pub(crate) const NAMES: [(&'static str, Self); 6] = [
        ("phishing", Self::Phishing),
        ("malware", Self::Malware),
        ("illegal", Self::Illegal),
        ("impersonation", Self::Impersonation),
        ("spam", Self::Spam),
        ("other", Self::Other),
    ];

    /// The text this kind is stored and shown as.
    pub fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("other", |(name, _)| name)
    }
}

impl fmt::Display for Abuse {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// Where a report stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ReportState {
    /// Nobody has acted on it.
    Open,
    /// The operator suspended the site because of it.
    Actioned,
}

impl ReportState {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 2] = [("open", Self::Open), ("actioned", Self::Actioned)];

    /// The text this state is stored as.
    pub fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("open", |(name, _)| name)
    }
}

/// A report's text, or a contact, that does not fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReportError {
    /// There is nothing in it.
    Empty {
        /// Which field.
        field: &'static str,
    },
    /// It is longer than the most the field takes.
    Long {
        /// Which field.
        field: &'static str,
        /// The most characters.
        limit: usize,
        /// The characters it has.
        found: usize,
    },
    /// It holds a control character other than a line break.
    Control {
        /// Which field.
        field: &'static str,
    },
}

impl fmt::Display for ReportError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { field } => write!(out, "the {field} of a report is not empty"),
            Self::Long { field, limit, found } => {
                write!(out, "the {field} of a report is at most {limit} characters, not {found}")
            }
            Self::Control { field } => write!(out, "the {field} of a report holds a control character"),
        }
    }
}

impl std::error::Error for ReportError {}

impl ReportError {
    /// Reads `text` as the `field` of a report of at most `limit` characters.
    pub(crate) fn bound(field: &'static str, limit: usize, text: &str) -> Result<String, Self> {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return Err(Self::Empty { field });
        }
        let found = text.chars().count();
        if found > limit {
            return Err(Self::Long { field, limit, found });
        }
        if text.chars().any(|c| c.is_control() && c != '\n') {
            return Err(Self::Control { field });
        }
        Ok(text)
    }
}

/// What a reporter says happened: bounded text with no control characters but line breaks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Detail(String);

impl Detail {
    /// The most characters.
    pub const LONGEST: usize = 2000;

    /// The text, as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Detail {
    type Error = ReportError;

    fn try_from(text: String) -> Result<Self, ReportError> {
        ReportError::bound("detail", Self::LONGEST, &text).map(Self)
    }
}

/// How the reporter may be reached: an address or a name, kept for the operator and never shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Contact(String);

impl Contact {
    /// The most characters.
    pub const LONGEST: usize = 200;

    /// The text, as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Contact {
    type Error = ReportError;

    fn try_from(text: String) -> Result<Self, ReportError> {
        ReportError::bound("contact", Self::LONGEST, &text).map(Self)
    }
}

/// A report to record. The caller draws the id and gives the time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewReport {
    /// The report.
    pub id: Id<ReportKind>,
    /// The handle the report names. It need not be a site.
    pub handle: Handle,
    /// What it is about.
    pub kind: Abuse,
    /// What happened.
    pub detail: Detail,
    /// How to reach the reporter.
    pub contact: Option<Contact>,
    /// When it came.
    pub at: Timestamp,
}

/// A recorded report.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Report {
    /// The report.
    pub id: Id<ReportKind>,
    /// The handle it names.
    pub handle: Handle,
    /// What it is about.
    pub kind: Abuse,
    /// What happened.
    pub detail: Detail,
    /// How to reach the reporter.
    pub contact: Option<Contact>,
    /// Where it stands.
    pub state: ReportState,
    /// When it came.
    pub at: Timestamp,
}

impl Report {
    /// The columns, in the order [`Report::read`] reads them.
    pub(crate) const COLUMNS: [table::Report; 7] = [
        table::Report::Id,
        table::Report::Handle,
        table::Report::Kind,
        table::Report::Detail,
        table::Report::Contact,
        table::Report::State,
        table::Report::At,
    ];

    /// A report from its row. What is stored was checked when it went in, and is read as text.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let contact = fields.maybe_text(table::Report::Contact)?;
        Ok(Self {
            id: fields.id(table::Report::Id)?,
            handle: Handle::stored(row, table::Report::Handle)?,
            kind: fields.name(table::Report::Kind, &Abuse::NAMES)?,
            detail: Detail(fields.text(table::Report::Detail)?),
            contact: contact.map(Contact),
            state: fields.name(table::Report::State, &ReportState::NAMES)?,
            at: fields.time(table::Report::At)?,
        })
    }

    /// The statement that records a new report, open.
    pub(crate) fn insert(new: &NewReport) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Report::Table)
            .columns(Self::COLUMNS)
            .values([
                Expr::val(new.id.to_string()),
                Expr::val(new.handle.as_str()),
                Expr::val(new.kind.name()),
                Expr::val(new.detail.as_str()),
                Expr::val(new.contact.as_ref().map(|contact| contact.as_str().to_owned())),
                Expr::val(ReportState::Open.name()),
                Expr::val(new.at.as_millisecond()),
            ])?
            .to_owned()
            .into())
    }
}

/// A site the operator took down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suspension {
    /// The site.
    pub site: Id<Site>,
    /// Why.
    pub kind: Abuse,
    /// The label of the operator key that did it.
    pub by: String,
    /// The report that led to it, if one did.
    pub report: Option<Id<ReportKind>>,
    /// When.
    pub at: Timestamp,
}

impl Suspension {
    /// The columns, in the order [`Suspension::read`] reads them.
    pub(crate) const COLUMNS: [table::Suspension; 5] = [
        table::Suspension::SiteId,
        table::Suspension::Kind,
        table::Suspension::By,
        table::Suspension::ReportId,
        table::Suspension::At,
    ];

    /// A suspension from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            site: fields.id(table::Suspension::SiteId)?,
            kind: fields.name(table::Suspension::Kind, &Abuse::NAMES)?,
            by: fields.text(table::Suspension::By)?,
            report: fields.maybe_id(table::Suspension::ReportId)?,
            at: fields.time(table::Suspension::At)?,
        })
    }

    /// The statement that records this suspension.
    pub(crate) fn insert(&self) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Suspension::Table)
            .columns(Self::COLUMNS)
            .values([
                Expr::val(self.site.to_string()),
                Expr::val(self.kind.name()),
                Expr::val(self.by.as_str()),
                Expr::val(self.report.map(|report| report.to_string())),
                Expr::val(self.at.as_millisecond()),
            ])?
            .to_owned()
            .into())
    }
}
