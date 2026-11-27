//! A source plugin loaded as a component, adapted to what the host reads a source with:
//! [`Component<Source>::read`] gives the same [`Read`] as [`host_native::Repository::read`], the
//! snapshot made by the plugin's rule over the host's `git` capability (decision 0046).
//!
//! The plugin sees no file's bytes. The host's capability reads the clone's tree into a store
//! of its own while the plugin runs, and the files are moved into the caller's store once the
//! plugin has returned a snapshot, so a plugin that fails leaves the caller's store as it was.
//! A source gets a budget of its own: a fetch takes longer than a page's render, and the time the
//! host spends in `git.fetch` counts against the plugin's call: the fetch is cut at the call's
//! wall-clock deadline, since fuel and epochs do not run while the host does. The terms' tick must
//! be the runtime's, since the epoch advances on the runtime's ticker.
//!
//! The host checks what the plugin returns: a fetch may only be of the want's repository,
//! revision and grant, a read only of a clone the call fetched, and the snapshot given back must
//! be of the commit the host read and name only files the host stored.

use std::cell::RefCell;
use std::fmt;

use engine::Blobs as _;
use host_native::{Fetch, Read, Store, StoreError};
use model::git::{History, Refused, Want};
use model::{Diagnostic, Hash, Oid, Problem, Snapshot, Version};
use wasmtime::component::Linker;

use crate::bind::source::{Source as Sources, SourcePre, exports::source as served};
use crate::budget::Budget;
use crate::capability::{Sourcing, State};
use crate::component::Component;
use crate::error::{Error, Failure};
use crate::mirror::{Malformed, Seen};
use crate::runtime::{Runtime, Machine};

/// What a source's calls run with: the host's fetching and a budget of their own.
#[derive(Clone, Debug)]
pub struct Terms {
    /// How the host fetches.
    pub fetch: Fetch,
    /// What one call may spend, the time of `git.fetch` included.
    pub budget: Budget,
}

/// The `ink:source` world, with the terms its calls run with.
pub struct Source {
    pre: SourcePre<State>,
    terms: Terms,
}

impl fmt::Debug for Source {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Source").field("terms", &self.terms).finish_non_exhaustive()
    }
}

/// What stands for a failure the host already said, with its causes, in the diagnostics kept aside.
#[derive(Debug)]
pub struct Told;

impl fmt::Display for Told {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the host said why in the diagnostics kept aside")
    }
}

impl std::error::Error for Told {}

/// Why a source plugin gave no snapshot.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unsourced {
    /// The call itself failed: a trap, a budget, a value the host could not send.
    Call(Failure),
    /// The plugin refused, or the host refused it.
    Refused(Refused<Told>),
    /// The want did not cross the boundary, or the plugin said what it was sent is no want.
    Malformed,
    /// The plugin's snapshot is of another commit than the one the host fetched.
    Commit {
        /// The commit fetched.
        fetched: Oid,
    },
    /// The snapshot names a file whose bytes the host never read.
    Blob {
        /// The file's hash.
        hash: Hash,
    },
    /// The plugin gave a snapshot the model refuses.
    Value {
        /// Why.
        source: Malformed,
    },
    /// The plugin returned without the host having read a snapshot.
    Unread,
    /// The files the host read could not be moved into the caller's store.
    Store(StoreError),
}

impl fmt::Display for Unsourced {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call(_) => out.write_str("the source plugin's call failed"),
            Self::Refused(refused) => write!(out, "{refused}"),
            Self::Malformed => out.write_str("the source plugin and the host disagree on what a want or a snapshot is"),
            Self::Commit { fetched } => write!(out, "the snapshot is not of the fetched commit {}", fetched.as_str()),
            Self::Blob { hash } => write!(out, "the snapshot names the file {hash}, which the host never read"),
            Self::Value { .. } => out.write_str("the source plugin gave a snapshot the model refuses"),
            Self::Unread => out.write_str("the source plugin gave a snapshot the host never read"),
            Self::Store(_) => out.write_str("the files the host read could not be stored"),
        }
    }
}

impl std::error::Error for Unsourced {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Call(source) => Some(source),
            Self::Refused(source) => Some(source),
            Self::Value { source } => Some(source),
            Self::Store(source) => Some(source),
            Self::Malformed | Self::Commit { .. } | Self::Blob { .. } | Self::Unread => None,
        }
    }
}

impl Unsourced {
    /// What a person sees: the host's own refusal as its catalog entry, the rest as the failure
    /// of the plugin `plugin`.
    pub fn diagnostic(&self, plugin: &str) -> Diagnostic {
        match self {
            Self::Call(failure) => failure.diagnostic(plugin),
            Self::Refused(refused) => Diagnostic::from(refused),
            Self::Malformed | Self::Commit { .. } | Self::Blob { .. } | Self::Unread | Self::Value { .. } => {
                Diagnostic::new(Problem::PluginValueMalformed { plugin: plugin.to_owned(), reason: self.to_string() })
            }
            Self::Store(error) => Diagnostic::new(Problem::SourceBlobUnwritable { reason: error.to_string() }),
        }
    }
}

impl Runtime {
    /// The source plugin `name` from the component in `bytes`, its calls run on `terms`.
    pub fn source(&self, name: &str, bytes: &[u8], terms: Terms) -> Result<Component<Source>, Error> {
        if terms.budget.tick != self.machine.budget.tick {
            return Err(Error::TermsTick { terms: terms.budget.tick, runtime: self.machine.budget.tick });
        }
        let (compiled, origin) = self.machine.compile(name, bytes)?;
        let mut linker = Linker::new(&self.machine.engine);
        let link = |source| Error::Link { plugin: name.to_owned(), source };
        Sources::add_to_linker::<State, wasmtime::component::HasSelf<State>>(&mut linker, |state| state)
            .map_err(link)?;
        let pre = linker.instantiate_pre(&compiled).and_then(SourcePre::new).map_err(link)?;
        Ok(Component {
            machine: self.machine.clone(),
            name: name.to_owned(),
            origin,
            world: Source { pre, terms },
            aside: RefCell::default(),
        })
    }
}

impl Component<Source> {
    /// The snapshot of the repository `want` names, as the plugin's rule makes it from the host's
    /// `git` capability, its files stored in `store`. What the call says is kept aside.
    pub fn read(&self, want: &Want, store: &mut impl Store) -> Result<Read, Unsourced> {
        let (source, terms) = (&self.world, &self.world.terms);
        let mut wasm = self.machine.store_in(
            &self.name,
            &terms.budget,
            Sourcing::bound(terms.fetch.clone(), want.clone(), terms.budget.time),
        );
        let wire = served::Want {
            locator: want.locator.to_string(),
            revision: want.revision.as_ref().map(ToString::to_string),
            history: matches!(want.history, History::Wanted),
            grant: want.grant.as_ref().map(ToString::to_string),
        };
        let called = Machine::call_in(&terms.budget, &mut wasm, |wasm| {
            source.pre.instantiate(&mut *wasm)?.source().call_read(wasm, &wire)
        });
        let state = wasm.into_data();
        let mut aside = self.aside.borrow_mut();
        aside.diagnostics.extend(state.said.diagnostics);
        aside.notices.extend(state.said.notices);
        drop(aside);
        let outcome = called.map_err(|failure| {
            self.fail(&failure);
            Unsourced::Call(failure)
        })?;
        let snapshot = outcome.map_err(Self::unsourced)?;
        let snapshot = Snapshot::try_from(snapshot).map_err(|source| Unsourced::Value { source })?;
        let Sourcing { blobs, read, .. } = state.sourcing;
        let mut read = read.ok_or(Unsourced::Unread)?;
        let Version::GitCommit(fetched) = read.snapshot.version() else { return Err(Unsourced::Unread) };
        if snapshot.version() != read.snapshot.version() {
            return Err(Unsourced::Commit { fetched: fetched.clone() });
        }
        if let Some(entry) = snapshot.tree().iter().find(|entry| blobs.read(&entry.blob).is_err()) {
            return Err(Unsourced::Blob { hash: entry.blob });
        }
        for bytes in blobs {
            store.put(bytes).map_err(Unsourced::Store)?;
        }
        read.snapshot = snapshot;
        Ok(read)
    }

    /// The plugin's refusal as the host sees it. What the host itself refused is the same typed
    /// refusal it gave; a failure it said already is carried as its diagnostics.
    fn unsourced(refused: served::Refused) -> Unsourced {
        match refused {
            served::Refused::Fetch(refusal) => Refused::try_from(Seen(refusal, Told))
                .map_or_else(|source| Unsourced::Value { source }, Unsourced::Refused),
            served::Refused::Malformed => Unsourced::Malformed,
            served::Refused::Commit(fetched) => {
                Oid::parse(fetched).map_or(Unsourced::Malformed, |fetched| Unsourced::Commit { fetched })
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_diagnostic_names_the_plugin_it_is_about() {
        let diagnostic = Unsourced::Malformed.diagnostic("ink:source-other");
        assert!(format!("{diagnostic:?}").contains("ink:source-other"), "{diagnostic:?}");
    }
}
