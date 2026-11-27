//! The requests the control plane refuses.

use std::fmt;

use model::{DeployId, Hash, Route};

use crate::id::{Account, Credential, Id, Site};
use crate::Handle;

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
