//! A preview of a branch: its label, the deploy it serves and when it ends (decision 0091).

mod migration;

use std::fmt;
use std::str::FromStr;

use db::{Row, Statement};
use jiff::Timestamp;
use model::{Bytes, DeployId, Manifest};
use sea_query::{Expr, Query};
use serde::Serialize;

use crate::fields::Fields;
use crate::id::{Id, Site};
use crate::{Unreadable, table};

pub use migration::{Pinned, Previewed};

/// The label of a preview, the part of its host before `--`: lower-case letters, digits and single
/// hyphens, not starting or ending with one. A handle has no `--`, so `<slug>--<handle>` is never
/// a handle, and a preview cannot be addressed as, or take the place of, a site.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Slug(String);

/// Text that is no preview label.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlugError {
    /// Nothing is left of it.
    Empty,
    /// It is longer than a host label has room for beside a handle.
    Long {
        /// The characters it has.
        found: usize,
    },
    /// A character that is not a lower-case letter, a digit or a hyphen.
    Character {
        /// Where, counted in characters from 0.
        at: usize,
    },
    /// It starts or ends with a hyphen, or holds two in a row, which would read as the `--`
    /// between label and handle.
    Hyphen,
}

impl fmt::Display for SlugError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("a preview label is not empty"),
            Self::Long { found } => write!(out, "a preview label is at most {} characters, not {found}", Slug::LONGEST),
            Self::Character { at } => {
                write!(out, "character {at} of the preview label is not a lower-case letter, a digit or a hyphen")
            }
            Self::Hyphen => out.write_str("a preview label does not start or end with a hyphen or hold two in a row"),
        }
    }
}

impl std::error::Error for SlugError {}

impl Slug {
    /// The most characters: a host label is 63, and the handle and the `--` take the rest.
    pub const LONGEST: usize = 29;

    /// The label as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The label a branch name makes: lower-cased, with every run of characters that are not
    /// letters or digits made one hyphen, trimmed, and cut at the longest. `feature/Login_v2`
    /// is `feature-login-v2`. Two branches can make the same label, and the later push replaces.
    pub fn branch(name: &str) -> Result<Self, SlugError> {
        let mut text = String::new();
        for c in name.chars().map(|c| c.to_ascii_lowercase()) {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                text.push(c);
            } else if !text.ends_with('-') {
                text.push('-');
            }
        }
        let text: String = text.trim_matches('-').chars().take(Self::LONGEST).collect();
        text.trim_end_matches('-').parse()
    }
}

impl FromStr for Slug {
    type Err = SlugError;

    fn from_str(text: &str) -> Result<Self, SlugError> {
        let found = text.chars().count();
        if found == 0 {
            return Err(SlugError::Empty);
        }
        if found > Self::LONGEST {
            return Err(SlugError::Long { found });
        }
        if let Some(at) = text.chars().position(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')) {
            return Err(SlugError::Character { at });
        }
        if text.starts_with('-') || text.ends_with('-') || text.contains("--") {
            return Err(SlugError::Hyphen);
        }
        Ok(Self(text.to_owned()))
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// A preview to record: the manifest of a built branch, whose blobs are in the site's tenant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPreview {
    /// The site.
    pub site: Id<Site>,
    /// The label, from the branch.
    pub slug: Slug,
    /// What the preview serves.
    pub manifest: Manifest,
    /// When it is made.
    pub at: Timestamp,
}

/// A preview that exists.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Preview {
    /// The site.
    pub site: Id<Site>,
    /// The label.
    pub slug: Slug,
    /// The deploy it serves, which no listing of deploys names.
    pub deploy: DeployId,
    /// How many files.
    pub files: u64,
    /// How many bytes they hold.
    pub bytes: Bytes,
    /// When it was made.
    pub created: Timestamp,
    /// When it ends.
    pub expires: Timestamp,
}

impl Preview {
    /// The columns, in the order [`Preview::read`] reads them.
    pub(crate) const COLUMNS: [table::Preview; 7] = [
        table::Preview::SiteId,
        table::Preview::Slug,
        table::Preview::DeployId,
        table::Preview::Files,
        table::Preview::Bytes,
        table::Preview::CreatedAt,
        table::Preview::ExpiresAt,
    ];

    /// A preview from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let text = fields.text(table::Preview::Slug)?;
        let slug = text.parse().map_err(|source| Unreadable::Slug { source })?;
        Ok(Self {
            site: fields.id(table::Preview::SiteId)?,
            slug,
            deploy: fields.deploy(table::Preview::DeployId)?,
            files: fields.count(table::Preview::Files)?,
            bytes: Bytes(fields.count(table::Preview::Bytes)?),
            created: fields.time(table::Preview::CreatedAt)?,
            expires: fields.time(table::Preview::ExpiresAt)?,
        })
    }

    /// Whether the preview has ended at `at`.
    pub fn is_over(&self, at: Timestamp) -> bool {
        self.expires <= at
    }

    /// The statement that records this preview, in place of one with the same label.
    pub(crate) fn replace(&self) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .replace()
            .into_table(table::Preview::Table)
            .columns(Self::COLUMNS)
            .values([
                Expr::val(self.site.to_string()),
                Expr::val(self.slug.as_str()),
                Expr::val(self.deploy.to_string()),
                Expr::val(i64::try_from(self.files).unwrap_or(i64::MAX)),
                Expr::val(i64::try_from(self.bytes.count()).unwrap_or(i64::MAX)),
                Expr::val(self.created.as_millisecond()),
                Expr::val(self.expires.as_millisecond()),
            ])?
            .to_owned()
            .into())
    }
}
