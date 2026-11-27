//! A git source the worker builds, fetched by the `ink:source-git` component (decision 0046):
//! the component decides what to fetch, the host's `git` capability fetches it within the
//! operator's network setting and limits, and the credential of the site is held for that one
//! call and named to the component, never shown to it.

use std::cell::RefCell;
use std::path::PathBuf;

use build::Request;
use build_local::Remote;
use control::{Control, Id, Ports, SiteKind};
use host_native::{Fetch, Memory, Read};
use host_wasmtime::{Budget, Cache, Component, Runtime, Source, Terms};
use model::git::{Grant, Network, Want};
use model::{Diagnostic, Problem};
use watch::Access;

use crate::bundle::Components;
use crate::error::Error;

/// What the fetch of a git source is set up with.
#[derive(Clone, Debug)]
pub(crate) struct Settings {
    /// Where clones are kept.
    pub(crate) cache: PathBuf,
    /// Where the compiled components are kept.
    pub(crate) components: PathBuf,
    /// What the builder may reach.
    pub(crate) network: Network,
    /// How an https remote is reached.
    pub(crate) https: https::Https,
    /// What one fetch and read may take, in wall time.
    pub(crate) budget: Budget,
}

/// Why a fetch failed in its own right.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum Unfetched {
    /// The source component is not held, or could not be read.
    Component(Box<Error>),
    /// The plugin host could not be made or the component not loaded.
    Plugin(host_wasmtime::Error),
    /// The site's id is not the one its tenant says.
    Tenant(String),
    /// The site's credential could not be opened.
    Access(Box<dyn std::error::Error>),
    /// A constant of this module is no longer valid.
    Gone,
}

impl std::fmt::Display for Unfetched {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Component(_) => out.write_str("the git source component is not available"),
            Self::Plugin(_) => out.write_str("the git source component could not be loaded"),
            Self::Tenant(tenant) => write!(out, "the tenant {tenant} names no site"),
            Self::Access(_) => out.write_str("the site's git credential could not be opened"),
            Self::Gone => out.write_str("the fetch is not set up as this build expects"),
        }
    }
}

impl std::error::Error for Unfetched {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Component(source) => Some(source.as_ref()),
            Self::Plugin(source) => Some(source),
            Self::Access(source) => Some(source.as_ref()),
            Self::Tenant(_) | Self::Gone => None,
        }
    }
}

/// The remote of the worker: the source component over the host's fetch.
pub(crate) struct Fetching<P: Ports> {
    control: Control<P>,
    settings: Settings,
    components: Components,
    runtime: RefCell<Option<Runtime>>,
}

impl<P: Ports + 'static> Fetching<P> {
    /// The component the repository is read with.
    const SOURCE: &'static str = "ink:source-git";
    /// The name a site's credential is held under for one call.
    const GRANT: &'static str = "site";

    pub(crate) fn new(control: Control<P>, settings: Settings) -> Self {
        Self { control, settings, components: Components::new(None), runtime: RefCell::new(None) }
    }

    /// The source component over `terms`, on a plugin host made on first use so a server without
    /// git sites never loads one.
    fn component(&self, terms: Terms) -> Result<Component<Source>, Unfetched> {
        let held = self.components.hold(Self::SOURCE).map_err(|error| Unfetched::Component(Box::new(error)))?;
        let mut slot = self.runtime.borrow_mut();
        if slot.is_none() {
            let cache = Cache::new(&self.settings.components);
            *slot = Some(Runtime::new(self.settings.budget, Some(cache)).map_err(Unfetched::Plugin)?);
        }
        let runtime = slot.as_ref().ok_or(Unfetched::Gone)?;
        runtime.source(Self::SOURCE, held.bytes(), terms).map_err(Unfetched::Plugin)
    }
}

impl<P: Ports + 'static> Remote for Fetching<P> {
    type Error = Unfetched;

    fn reaches(&self) -> bool {
        true
    }

    async fn fetch(&self, request: &Request, store: &mut Memory) -> Result<Result<Read, Box<Diagnostic>>, Unfetched> {
        let build::Source::Git { url, revision } = &request.source else {
            let reason = "not a git source".to_owned();
            return Ok(Err(Box::new(Diagnostic::new(Problem::SourceRepositoryUnreadable { reason }))));
        };
        let tenant = request.tenant.to_string();
        let site = tenant.parse::<Id<SiteKind>>().map_err(|_| Unfetched::Tenant(tenant))?;
        let access = Access::of(&self.control, site).await.map_err(Unfetched::Access)?;
        let mut fetch = Fetch {
            network: self.settings.network,
            https: self.settings.https.clone(),
            ..Fetch::new(&self.settings.cache)
        };
        let mut want = Want::new(url.clone());
        want.revision = revision.as_ref().and_then(|revision| revision.as_ref().parse().ok());
        if let Some(access) = &access {
            let grant: Grant = Self::GRANT.parse().map_err(|_| Unfetched::Gone)?;
            fetch.grants.hold(grant.clone(), access.user(), access.password());
            want.grant = Some(grant);
        }
        let component = self.component(Terms { fetch, budget: self.settings.budget })?;
        Ok(component.read(&want, store).map_err(|unsourced| Box::new(unsourced.diagnostic(Self::SOURCE))))
    }
}
