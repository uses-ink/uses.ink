//! What the control plane fails with.

use std::fmt;

use blob::Blob;
use db::Db;
use model::{DeployId, Hash, Route};
use queue::Queue;
use secret::Secret;

use crate::id::{Account, Credential, Id, IdError, Site};
use crate::{Handle, HandleError, Ports};

/// A request the control plane refuses, and the fact that makes it wrong. Nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// The handle is kept for uses.ink itself.
    Reserved {
        /// The handle.
        handle: Handle,
    },
    /// The handle belongs to a site.
    Taken {
        /// The handle.
        handle: Handle,
    },
    /// The handle belonged to a site that was removed, and stays unclaimable (architecture
    /// section 12).
    Tombstoned {
        /// The handle.
        handle: Handle,
    },
    /// A row with this id exists already.
    Exists {
        /// The id, as text.
        id: String,
    },
    /// No such account.
    NoAccount {
        /// The account.
        account: Id<Account>,
    },
    /// No such site, or it is being removed.
    NoSite {
        /// The site.
        site: Id<Site>,
    },
    /// The site is being removed and takes no change.
    Removing {
        /// The site.
        site: Id<Site>,
    },
    /// The deploy names a file whose blob the store does not hold.
    MissingBlob {
        /// The route.
        route: Route,
        /// The blob.
        blob: Hash,
    },
    /// The site has no such deploy.
    NoDeploy {
        /// The site.
        site: Id<Site>,
        /// The deploy.
        deploy: DeployId,
    },
    /// The site has no live deploy.
    NotLive {
        /// The site.
        site: Id<Site>,
    },
    /// The deploy is live already.
    AlreadyLive {
        /// The site.
        site: Id<Site>,
        /// The deploy.
        deploy: DeployId,
    },
    /// No such credential.
    NoCredential {
        /// The credential.
        credential: Id<Credential>,
    },
    /// The deploy's manifest is not in the blob store.
    NoManifest {
        /// The deploy.
        deploy: DeployId,
    },
    /// The site has no such build.
    NoBuild {
        /// The site.
        site: Id<Site>,
        /// The build.
        build: build::BuildId,
    },
    /// A build's log is not in the blob store.
    MissingLog {
        /// The build.
        build: build::BuildId,
        /// The blob.
        blob: Hash,
    },
    /// A source with a path that is not text cannot be stored.
    Unstorable,
    /// A site's id is not a valid tenant. Cannot happen for an id made from 16 bytes.
    Tenant(blob::TenantError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved { handle } => write!(out, "{handle} is reserved for uses.ink itself"),
            Self::Taken { handle } => write!(out, "{handle} is taken"),
            Self::Tombstoned { handle } => write!(out, "{handle} was used by a site that was removed and is not free"),
            Self::Exists { id } => write!(out, "a row with the id {id} exists already"),
            Self::NoAccount { account } => write!(out, "there is no account {account}"),
            Self::NoSite { site } => write!(out, "there is no site {site}"),
            Self::Removing { site } => write!(out, "the site {site} is being removed"),
            Self::MissingBlob { route, blob } => {
                write!(out, "{route} is served from {blob}, which the store does not hold")
            }
            Self::NoDeploy { site, deploy } => write!(out, "the site {site} has no deploy {deploy}"),
            Self::NotLive { site } => write!(out, "the site {site} has no live deploy"),
            Self::AlreadyLive { site, deploy } => write!(out, "the deploy {deploy} is live on {site} already"),
            Self::NoCredential { credential } => write!(out, "there is no credential {credential}"),
            Self::NoManifest { deploy } => write!(out, "the manifest of deploy {deploy} is not in the blob store"),
            Self::NoBuild { site, build } => write!(out, "the site {site} has no build {build}"),
            Self::MissingLog { build, blob } => {
                write!(out, "the log of build {build} is {blob}, which the store does not hold")
            }
            Self::Unstorable => out.write_str("a folder path that is not text cannot be stored"),
            Self::Tenant(_) => out.write_str("the site's id is not a valid tenant"),
        }
    }
}

impl std::error::Error for Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Tenant(source) => Some(source),
            _ => None,
        }
    }
}

/// A stored row that does not read back as the domain value it should be: the database was
/// changed by something other than this code, or by another version of it.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unreadable {
    /// The column is missing or of the wrong kind.
    Column(db::ColumnError),
    /// The column holds a bad id.
    Id {
        /// The column.
        column: &'static str,
        /// Why.
        source: IdError,
    },
    /// The column holds a bad handle.
    Handle {
        /// Why.
        source: HandleError,
    },
    /// The column holds a bad build id.
    Build {
        /// Why.
        source: build::BuildIdError,
    },
    /// The column holds a bad deploy id.
    Deploy {
        /// The column.
        column: &'static str,
        /// Why.
        source: serde::de::value::Error,
    },
    /// The column holds a bad hash.
    Hash {
        /// The column.
        column: &'static str,
        /// Why.
        source: model::HashError,
    },
    /// The column holds a time that does not exist.
    Time {
        /// The column.
        column: &'static str,
        /// Why.
        source: jiff::Error,
    },
    /// The column holds a name this version does not know, such as a state.
    Name {
        /// The column.
        column: &'static str,
        /// What it holds.
        found: String,
    },
    /// The column holds a kind of source this version does not know.
    Locator {
        /// The column.
        column: &'static str,
    },
    /// The column holds a URL that is no longer valid.
    Url {
        /// The column.
        column: &'static str,
        /// Why.
        source: model::UrlError,
    },
    /// The column holds a revision that is no longer valid.
    Revision {
        /// The column.
        column: &'static str,
        /// Why.
        source: build::RevisionError,
    },
    /// The column holds an email address that is no longer valid.
    Address {
        /// The column.
        column: &'static str,
        /// Why.
        source: mail::AddressError,
    },
    /// The column holds a digest that is not 32 bytes.
    Digest {
        /// The column.
        column: &'static str,
    },
}

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Column(_) => out.write_str("a stored row has a column that cannot be read"),
            Self::Id { column, .. } => write!(out, "column `{column}` holds a bad id"),
            Self::Handle { .. } => out.write_str("column `handle` holds a bad handle"),
            Self::Build { .. } => out.write_str("a stored build id is bad"),
            Self::Deploy { column, .. } => write!(out, "column `{column}` holds a bad deploy id"),
            Self::Hash { column, .. } => write!(out, "column `{column}` holds a bad hash"),
            Self::Time { column, .. } => write!(out, "column `{column}` holds a bad time"),
            Self::Name { column, found } => {
                write!(out, "column `{column}` holds `{found}`, which this version does not know")
            }
            Self::Locator { column } => {
                write!(out, "column `{column}` holds a kind of source this version does not know")
            }
            Self::Url { column, .. } => write!(out, "column `{column}` holds a URL that is no longer valid"),
            Self::Revision { column, .. } => write!(out, "column `{column}` holds a revision that is no longer valid"),
            Self::Address { column, .. } => write!(out, "column `{column}` holds an address that is no longer valid"),
            Self::Digest { column } => write!(out, "column `{column}` holds a digest that is not 32 bytes"),
        }
    }
}

impl std::error::Error for Unreadable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Column(source) => Some(source),
            Self::Id { source, .. } => Some(source),
            Self::Handle { source } => Some(source),
            Self::Build { source } => Some(source),
            Self::Deploy { source, .. } => Some(source),
            Self::Hash { source, .. } => Some(source),
            Self::Time { source, .. } => Some(source),
            Self::Url { source, .. } => Some(source),
            Self::Revision { source, .. } => Some(source),
            Self::Address { source, .. } => Some(source),
            Self::Name { .. } | Self::Locator { .. } | Self::Digest { .. } => None,
        }
    }
}

impl From<db::ColumnError> for Unreadable {
    fn from(source: db::ColumnError) -> Self {
        Self::Column(source)
    }
}

/// A control plane call that failed. `P` is the ports it ran on.
#[non_exhaustive]
pub enum Error<P: Ports> {
    /// The request is refused.
    Refused(Refusal),
    /// The database failed.
    Db(db::Error<<P::Db as Db>::Error>),
    /// The blob store failed.
    Blob(blob::Error<<P::Blob as Blob>::Error>),
    /// The queue failed.
    Queue(queue::Error<<P::Queue as Queue>::Error>),
    /// The sealer failed.
    Secret(secret::Error<<P::Secret as Secret>::Error>),
    /// A stored row is not what this version writes.
    Unreadable(Unreadable),
    /// A build's log or a site event could not be written as JSON.
    Log(serde_json::Error),
    /// A statement could not be built: its columns and values do not match.
    Statement(sea_query::error::Error),
    /// A stored manifest is not in the canonical form.
    Manifest(crate::listed::Canonical),
    /// A site's address could not be made from the origin.
    Origin(model::UrlError),
}

impl<P: Ports> fmt::Debug for Error<P> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(error) => out.debug_tuple("Refused").field(error).finish(),
            Self::Db(error) => out.debug_tuple("Db").field(error).finish(),
            Self::Blob(error) => out.debug_tuple("Blob").field(error).finish(),
            Self::Queue(error) => out.debug_tuple("Queue").field(error).finish(),
            Self::Secret(error) => out.debug_tuple("Secret").field(error).finish(),
            Self::Unreadable(error) => out.debug_tuple("Unreadable").field(error).finish(),
            Self::Log(error) => out.debug_tuple("Log").field(error).finish(),
            Self::Statement(error) => out.debug_tuple("Statement").field(error).finish(),
            Self::Manifest(error) => out.debug_tuple("Manifest").field(error).finish(),
            Self::Origin(error) => out.debug_tuple("Origin").field(error).finish(),
        }
    }
}

impl<P: Ports> fmt::Display for Error<P> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => refusal.fmt(out),
            Self::Db(_) => out.write_str("the database failed"),
            Self::Blob(_) => out.write_str("the blob store failed"),
            Self::Queue(_) => out.write_str("the queue failed"),
            Self::Secret(_) => out.write_str("sealing failed"),
            Self::Unreadable(_) => out.write_str("a stored row is not what this version writes"),
            Self::Log(_) => out.write_str("a build's log could not be written"),
            Self::Statement(_) => out.write_str("a statement could not be built"),
            Self::Manifest(_) => out.write_str("a stored manifest is not in the canonical form"),
            Self::Origin(_) => out.write_str("a site's address could not be made from the origin"),
        }
    }
}

impl<P: Ports> std::error::Error for Error<P> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(source) => Some(source),
            Self::Db(source) => Some(source),
            Self::Blob(source) => Some(source),
            Self::Queue(source) => Some(source),
            Self::Secret(source) => Some(source),
            Self::Unreadable(source) => Some(source),
            Self::Log(source) => Some(source),
            Self::Statement(source) => Some(source),
            Self::Manifest(source) => Some(source),
            Self::Origin(source) => Some(source),
        }
    }
}

impl<P: Ports> From<sea_query::error::Error> for Error<P> {
    fn from(source: sea_query::error::Error) -> Self {
        Self::Statement(source)
    }
}

impl<P: Ports> From<crate::event::Unwritten> for Error<P> {
    fn from(error: crate::event::Unwritten) -> Self {
        match error {
            crate::event::Unwritten::Json(source) => Self::Log(source),
            crate::event::Unwritten::Statement(source) => Self::Statement(source),
        }
    }
}

impl<P: Ports> From<Refusal> for Error<P> {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

impl<P: Ports> From<Unreadable> for Error<P> {
    fn from(error: Unreadable) -> Self {
        Self::Unreadable(error)
    }
}

impl<P: Ports> From<db::ColumnError> for Error<P> {
    fn from(error: db::ColumnError) -> Self {
        Self::Unreadable(error.into())
    }
}

impl<P: Ports> From<db::Error<<P::Db as Db>::Error>> for Error<P> {
    fn from(error: db::Error<<P::Db as Db>::Error>) -> Self {
        Self::Db(error)
    }
}

impl<P: Ports> From<blob::Error<<P::Blob as Blob>::Error>> for Error<P> {
    fn from(error: blob::Error<<P::Blob as Blob>::Error>) -> Self {
        Self::Blob(error)
    }
}

impl<P: Ports> From<queue::Error<<P::Queue as Queue>::Error>> for Error<P> {
    fn from(error: queue::Error<<P::Queue as Queue>::Error>) -> Self {
        Self::Queue(error)
    }
}

impl<P: Ports> From<secret::Error<<P::Secret as Secret>::Error>> for Error<P> {
    fn from(error: secret::Error<<P::Secret as Secret>::Error>) -> Self {
        Self::Secret(error)
    }
}

impl<P: Ports> Error<P> {
    /// The refusal this is, if the request was refused rather than failed.
    pub const fn refusal(&self) -> Option<&Refusal> {
        match self {
            Self::Refused(refusal) => Some(refusal),
            _ => None,
        }
    }
}

/// A call that ran on the control plane's ports and on one more: the builder, or the sink an
/// export writes to. `X` is that one's own error.
#[non_exhaustive]
pub enum Failed<P: Ports, X> {
    /// The control plane failed or refused.
    Control(Error<P>),
    /// The builder or the sink failed.
    Other(X),
}

impl<P: Ports, X: fmt::Debug> fmt::Debug for Failed<P, X> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(error) => out.debug_tuple("Control").field(error).finish(),
            Self::Other(error) => out.debug_tuple("Other").field(error).finish(),
        }
    }
}

impl<P: Ports, X> fmt::Display for Failed<P, X> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(_) => out.write_str("the control plane failed"),
            Self::Other(_) => out.write_str("the builder or the sink failed"),
        }
    }
}

impl<P: Ports + 'static, X: std::error::Error + 'static> std::error::Error for Failed<P, X> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Control(source) => Some(source),
            Self::Other(source) => Some(source),
        }
    }
}

impl<P: Ports, X> From<Error<P>> for Failed<P, X> {
    fn from(error: Error<P>) -> Self {
        Self::Control(error)
    }
}
