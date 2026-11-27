//! Sites: one published blog, owned by an account, named by a handle.

use build::{Kind, Revision, Source};
use db::Row;
use jiff::Timestamp;
use model::git::Locator;
use sea_query::IdenStatic;

use crate::fields::Fields;
use crate::id::{Account, Id, Site as SiteKind};
use crate::table;
use crate::{Handle, Refusal, Unreadable};

/// Whether a site takes changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum State {
    /// Serving, building, taking changes.
    Active,
    /// A removal began and has not finished: its bytes may be partly gone. Removing again
    /// finishes it.
    Removing,
}

impl State {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 2] = [("active", Self::Active), ("removing", Self::Removing)];

    /// The text this state is stored as, and the API writes.
    pub fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("active", |(name, _)| name)
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.name())
    }
}

/// One published blog.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Site {
    /// Its id.
    pub id: Id<SiteKind>,
    /// Who owns it.
    pub account: Id<Account>,
    /// The name it is served under.
    pub handle: Handle,
    /// Whether it takes changes.
    pub state: State,
    /// When it was made.
    pub created: Timestamp,
    /// Where its content comes from.
    pub source: Source,
}

/// A site to add.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct New {
    /// Its id, drawn by the caller.
    pub id: Id<SiteKind>,
    /// Who owns it.
    pub account: Id<Account>,
    /// The name to claim.
    pub handle: Handle,
    /// Where its content comes from.
    pub source: Source,
    /// What time it is.
    pub at: Timestamp,
}

/// A source as its row stores it: a kind, a locator and a revision.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Stored {
    pub(crate) kind: Kind,
    pub(crate) locator: String,
    pub(crate) revision: Option<String>,
}

impl Stored {
    /// The kinds the column stores.
    pub(crate) const KINDS: [(&'static str, Kind); 3] =
        [("folder", Kind::Folder), ("git", Kind::Git), ("upload", Kind::Upload)];

    /// The text a kind is stored as.
    pub(crate) fn name(kind: Kind) -> &'static str {
        Self::KINDS.iter().find(|(_, found)| *found == kind).map_or("folder", |(name, _)| name)
    }
}

impl TryFrom<&Source> for Stored {
    type Error = Refusal;

    fn try_from(source: &Source) -> Result<Self, Refusal> {
        match source {
            Source::Folder(path) => Ok(Self {
                kind: Kind::Folder,
                locator: path.to_str().ok_or(Refusal::Unstorable)?.to_owned(),
                revision: None,
            }),
            Source::Git { url, revision } => Ok(Self {
                kind: Kind::Git,
                locator: url.to_string(),
                revision: revision.as_ref().map(|revision| revision.as_str().to_owned()),
            }),
            Source::Upload => Ok(Self { kind: Kind::Upload, locator: String::new(), revision: None }),
            _ => Err(Refusal::Unstorable),
        }
    }
}

impl TryFrom<&Row> for Stored {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            kind: fields.name(table::Source::Kind, &Self::KINDS)?,
            locator: fields.text(table::Source::Locator)?,
            revision: row.get(table::Source::Revision)?,
        })
    }
}

impl TryFrom<Stored> for Source {
    type Error = Unreadable;

    fn try_from(stored: Stored) -> Result<Self, Unreadable> {
        match stored.kind {
            Kind::Folder => Ok(Self::Folder(stored.locator.into())),
            Kind::Upload => Ok(Self::Upload),
            Kind::Git => {
                let url = stored
                    .locator
                    .parse::<Locator>()
                    .map_err(|source| Unreadable::Url { column: table::Source::Locator.as_str(), source })?;
                let revision = stored
                    .revision
                    .map(|text| {
                        text.parse::<Revision>()
                            .map_err(|source| Unreadable::Revision { column: table::Source::Revision.as_str(), source })
                    })
                    .transpose()?;
                Ok(Self::Git { url, revision })
            }
            _ => Err(Unreadable::Locator { column: table::Source::Kind.as_str() }),
        }
    }
}

impl Site {
    /// A site from its row joined with its source's.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            id: fields.id(table::Site::Id)?,
            account: fields.id(table::Site::AccountId)?,
            handle: Handle::stored(row, table::Site::Handle)?,
            state: fields.name(table::Site::State, &State::NAMES)?,
            created: fields.time(table::Site::CreatedAt)?,
            source: Source::try_from(Stored::try_from(row)?)?,
        })
    }
}
