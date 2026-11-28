//! Deploys, the pointer a site serves from, and the history of its moves.

use build::BuildId;
use db::Row;
use jiff::Timestamp;
use model::{Bytes, DeployId, Manifest};

use crate::Unreadable;
use crate::fields::Fields;
use crate::id::{Id, Site};
use crate::table;

/// A deploy to record: a manifest whose every blob is already in the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct New {
    /// The site.
    pub site: Id<Site>,
    /// The manifest the build produced.
    pub manifest: Manifest,
    /// The build that made it, if one did.
    pub build: Option<BuildId>,
    /// What time it is.
    pub at: Timestamp,
}

/// One recorded deploy of a site.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Deploy {
    /// Its id: the hash of its manifest.
    pub id: DeployId,
    /// When it was recorded.
    pub created: Timestamp,
    /// The build that made it, if one did.
    pub build: Option<BuildId>,
    /// How many files it serves.
    pub files: u64,
    /// How many bytes they hold.
    pub bytes: Bytes,
    /// Whether the site serves it now.
    pub is_live: bool,
}

impl TryFrom<&Row> for Deploy {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let build: Option<String> = row.get(table::Deploy::JobId)?;
        Ok(Self {
            id: fields.deploy(table::Deploy::Id)?,
            created: fields.time(table::Deploy::CreatedAt)?,
            build: build.map(|text| text.parse().map_err(|source| Unreadable::Build { source })).transpose()?,
            files: fields.count(table::Deploy::Files)?,
            bytes: Bytes(fields.count(table::Deploy::Bytes)?),
            is_live: fields.flag(table::Computed::Live)?,
        })
    }
}

/// What moved the pointer, as the publication column stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    /// A build was published.
    Publish,
    /// A rollback.
    Rollback,
}

impl Cause {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 2] = [("publish", Self::Publish), ("rollback", Self::Rollback)];

    /// The text this cause is stored as.
    pub(crate) fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("publish", |(name, _)| name)
    }
}

/// What a deploy did to the site's pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Published {
    /// The deploy is live now, replacing the previous one, if there was one.
    Live {
        /// What it replaced.
        previous: Option<DeployId>,
    },
    /// The deploy was live already: building the same site again gives the same deploy.
    Unchanged,
    /// Publishing is paused (a rollback paused it until `resume`), so the deploy is recorded and
    /// reachable as a preview, and the pointer did not move.
    Held,
}

/// A deploy recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recorded {
    /// The deploy.
    pub deploy: DeployId,
    /// What it did to the pointer.
    pub published: Published,
}

/// The pointer a site serves from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Live {
    /// The deploy served now.
    pub deploy: DeployId,
    /// The one before it, which a rollback goes back to.
    pub previous: Option<DeployId>,
    /// Whether automatic publishing is paused.
    pub paused: bool,
    /// When the pointer last moved.
    pub since: Timestamp,
}

impl TryFrom<&Row> for Live {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            deploy: fields.deploy(table::Live::DeployId)?,
            previous: fields.maybe_deploy(table::Live::PreviousId)?,
            paused: fields.flag(table::Live::Paused)?,
            since: fields.time(table::Live::SetAt)?,
        })
    }
}

/// What a resume did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Resumed {
    /// Publishing was paused and is not now.
    Resumed,
    /// It was not paused.
    NotPaused,
}
