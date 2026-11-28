//! The requests the control plane refuses.

use std::fmt;

use jiff::Timestamp;
use model::{Bytes, DeployId, Hash, Route};

use crate::hold::Why;
use crate::id::{Account, Credential, Id, Site};
use crate::{Handle, Reservation, Spent};

/// A request the control plane refuses, and the fact that makes it wrong. Nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// The handle is kept: for uses.ink itself, or on the brand list.
    Reserved {
        /// The handle.
        handle: Handle,
        /// Which list it is on.
        reservation: Reservation,
    },
    /// The handle belongs to a site.
    Taken {
        /// The handle.
        handle: Handle,
    },
    /// The handle was left by a site and is held (decision 0087): for a time, or for good when
    /// its owner was verified.
    Held {
        /// The handle.
        handle: Handle,
        /// Why it is held.
        why: Why,
        /// When the hold ends. `None` is for good.
        until: Option<Timestamp>,
    },
    /// The site was suspended by the operator and takes no changes (decision 0089).
    Suspended {
        /// The site.
        site: Id<Site>,
        /// Why.
        kind: crate::AbuseKind,
    },
    /// The site is not suspended.
    Serving {
        /// The site.
        site: Id<Site>,
    },
    /// The site has no git source, so it has no branches to preview.
    NoPreview {
        /// The site.
        site: Id<Site>,
    },
    /// The site's repository is private: its branches are never served at an address anyone can
    /// open (decision 0091).
    Private {
        /// The site.
        site: Id<Site>,
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
    /// A budget is spent: the request would take the account over a limit (decision 0088).
    Over {
        /// Which budget.
        spent: Spent,
        /// What the account has or would have: bytes for the stored budget, a count for the others.
        value: u64,
        /// The limit it is over, in the same unit.
        limit: u64,
    },
    /// A source with a path that is not text cannot be stored.
    Unstorable,
    /// A site's id is not a valid tenant. Cannot happen for an id made from 16 bytes.
    Tenant(blob::TenantError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved { handle, reservation: Reservation::Infrastructure } => {
                write!(out, "{handle} is reserved for uses.ink itself")
            }
            Self::Reserved { handle, .. } => write!(out, "{handle} is a brand name that uses.ink keeps"),
            Self::Taken { handle } => write!(out, "{handle} is taken"),
            Self::Held { handle, until: Some(until), .. } => {
                write!(out, "{handle} was left by a site and is held until {until}")
            }
            Self::Held { handle, until: None, .. } => {
                write!(out, "{handle} belonged to a verified owner and is never given to anyone else")
            }
            Self::Suspended { site, kind } => write!(out, "the site {site} is suspended for {kind}"),
            Self::Serving { site } => write!(out, "the site {site} is not suspended"),
            Self::NoPreview { site } => {
                write!(out, "the site {site} has no git source, so it has no branches to preview")
            }
            Self::Private { site } => {
                write!(out, "the repository of site {site} is private, so its branches are not previewed")
            }
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
            Self::Over { spent, value, limit } => Self::over(out, *spent, (*value, *limit)),
            Self::Unstorable => out.write_str("a folder path that is not text cannot be stored"),
            Self::Tenant(_) => out.write_str("the site's id is not a valid tenant"),
        }
    }
}

impl Refusal {
    /// Words a refusal for going over a budget.
    fn over(out: &mut fmt::Formatter<'_>, spent: Spent, reached: (u64, u64)) -> fmt::Result {
        let (value, limit) = reached;
        match spent {
            Spent::Stored => {
                write!(out, "the account would store {} over its limit of {}", Bytes(value), Bytes(limit))
            }
            Spent::Builds => write!(out, "the account started {value} builds in a day, which is its limit of {limit}"),
            Spent::Sites => write!(out, "the account has {value} sites, which is its limit of {limit}"),
        }
    }

    /// The code of the catalog (`crates/core/diag/catalog.toml`) that words this refusal.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Reserved { reservation: Reservation::Infrastructure, .. } => "handle.name.reserved",
            Self::Reserved { .. } => "handle.name.brand",
            Self::Taken { .. } => "handle.name.taken",
            Self::Held { until: None, .. } => "handle.name.kept",
            Self::Held { .. } => "handle.name.held",
            Self::Suspended { .. } => "site.state.suspended",
            Self::Serving { .. } => "site.state.serving",
            Self::NoPreview { .. } => "preview.site.unsupported",
            Self::Private { .. } => "preview.site.private",
            Self::Exists { .. } => "row.id.exists",
            Self::NoAccount { .. } => "account.id.unknown",
            Self::NoSite { .. } => "site.id.unknown",
            Self::Removing { .. } => "site.state.removing",
            Self::MissingBlob { .. } => "deploy.blob.missing",
            Self::NoDeploy { .. } => "deploy.id.unknown",
            Self::NotLive { .. } => "site.deploy.absent",
            Self::AlreadyLive { .. } => "deploy.state.live",
            Self::NoCredential { .. } => "credential.id.unknown",
            Self::NoManifest { .. } => "deploy.manifest.missing",
            Self::NoBuild { .. } => "build.id.unknown",
            Self::MissingLog { .. } => "build.log.missing",
            Self::Over { spent: Spent::Stored, .. } => "budget.stored.over",
            Self::Over { spent: Spent::Builds, .. } => "budget.builds.over",
            Self::Over { spent: Spent::Sites, .. } => "budget.sites.over",
            Self::Unstorable => "source.path.unstorable",
            Self::Tenant(_) => "site.tenant.invalid",
        }
    }

    /// The data of the refusal, named as the catalog names the fields of its code.
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Reserved { handle, .. } | Self::Taken { handle } => vec![("handle", handle.to_string())],
            Self::Held { handle, until, .. } => {
                let mut fields = vec![("handle", handle.to_string())];
                fields.extend(until.map(|until| ("until", until.to_string())));
                fields
            }
            Self::Suspended { site, kind } => vec![("id", site.to_string()), ("reason", kind.to_string())],
            Self::Exists { id } => vec![("id", id.clone())],
            Self::NoAccount { account } => vec![("id", account.to_string())],
            Self::NoSite { site }
            | Self::Removing { site }
            | Self::NotLive { site }
            | Self::Serving { site }
            | Self::NoPreview { site }
            | Self::Private { site } => {
                vec![("id", site.to_string())]
            }
            Self::NoDeploy { site, deploy } | Self::AlreadyLive { site, deploy } => {
                vec![("id", site.to_string()), ("deploy", deploy.to_string())]
            }
            Self::MissingBlob { route, blob } => vec![("route", route.to_string()), ("hash", blob.to_string())],
            Self::NoCredential { credential } => vec![("id", credential.to_string())],
            Self::NoManifest { deploy } => vec![("deploy", deploy.to_string())],
            Self::NoBuild { site, build } => vec![("id", site.to_string()), ("build", build.to_string())],
            Self::MissingLog { build, blob } => vec![("build", build.to_string()), ("hash", blob.to_string())],
            Self::Over { spent, value, limit } => {
                let name = if *spent == Spent::Stored { "size" } else { "count" };
                vec![(name, value.to_string()), ("limit", limit.to_string())]
            }
            Self::Unstorable | Self::Tenant(_) => Vec::new(),
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
