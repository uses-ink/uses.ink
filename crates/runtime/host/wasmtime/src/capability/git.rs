use std::fmt;
use std::time::{Duration, Instant};

use engine::Sink as _;
#[cfg(test)]
use host_native::Grammars;
use host_native::{Failed, Fetch, Memory, Read};
#[cfg(test)]
use model::Oid;
use model::git::{Cloned, Refused, Request, Want};
use model::{Diagnostic, Hash, Problem};

use super::State;
use crate::bind::plugin::ink::host::git;
use crate::bind::plugin::ink::model as types;
#[cfg(test)]
use crate::budget::Budget;

/// What a source's calls to the `git` capability do and leave behind: the host's fetching, the
/// files of the last snapshot, and the read that made them. A source may fetch only the
/// repository, revision and grant of its [`Want`], and read only a clone it fetched itself; a
/// plugin that is not a source holds no want, so every fetch it asks for is refused.
#[derive(Debug)]
pub(crate) struct Sourcing {
    pub(crate) fetch: Fetch,
    pub(crate) blobs: Memory,
    pub(crate) read: Option<Read>,
    want: Option<Want>,
    deadline: Option<Instant>,
    fetched: Vec<Cloned>,
}

impl Default for Sourcing {
    fn default() -> Self {
        Self {
            fetch: Fetch::new(std::path::PathBuf::new()),
            blobs: Memory::default(),
            read: None,
            want: None,
            deadline: None,
            fetched: Vec::new(),
        }
    }
}

/// What part of a request the want does not allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Locator,
    Revision,
    Grant,
}

/// Why the host refused a `git` call that is not about the source's want.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Unbound {
    /// The plugin is no source, so it has no want to fetch.
    NoWant,
    /// The request differs from the want in this field.
    Field(Field),
    /// The clone is not one this call fetched.
    Clone(Hash),
}

impl fmt::Display for Unbound {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWant => out.write_str("the plugin has no want, so it may fetch nothing"),
            Self::Field(field) => {
                write!(out, "the request's {field:?} is not the want's: a source fetches only its want")
            }
            Self::Clone(clone) => write!(out, "the clone {clone} was not fetched by this call"),
        }
    }
}

impl std::error::Error for Unbound {}

impl Sourcing {
    /// Fetching as `fetch` says, for `want` only, until `time` from now has passed.
    pub(crate) fn bound(fetch: Fetch, want: Want, time: Duration) -> Self {
        Self { fetch, want: Some(want), deadline: Instant::now().checked_add(time), ..Self::default() }
    }

    /// Whether `request` asks for what the want does.
    fn allows(&self, request: &Request) -> Result<(), Unbound> {
        let want = self.want.as_ref().ok_or(Unbound::NoWant)?;
        if request.locator != want.locator {
            return Err(Unbound::Field(Field::Locator));
        }
        if request.revision != want.revision {
            return Err(Unbound::Field(Field::Revision));
        }
        if request.grant != want.grant {
            return Err(Unbound::Field(Field::Grant));
        }
        Ok(())
    }

    /// Whether `cloned` is a clone this call fetched, at the commit the fetch named.
    fn owns(&self, cloned: &Cloned) -> Result<(), Unbound> {
        let held = self.fetched.iter().any(|held| held.clone == cloned.clone && held.commit == cloned.commit);
        if held { Ok(()) } else { Err(Unbound::Clone(cloned.clone)) }
    }
}

impl State {
    /// A refusal of the host's fetching, as the plugin sees it. A failure with no case of its own
    /// is said here, once, with its causes; the typed refusals carry their numbers across.
    fn refuse(&mut self, refused: &Refused<Failed>) -> git::Refusal {
        if matches!(refused, Refused::Failed(_)) {
            self.said.diagnose(Diagnostic::from(refused));
        }
        git::Refusal::try_from(refused).unwrap_or_else(|unmirrored| {
            self.said.diagnose(Diagnostic::new(Problem::PluginValueUnmirrored { reason: unmirrored.to_string() }));
            git::Refusal::Failed
        })
    }
}

impl git::Host for State {
    fn fetch(&mut self, request: git::Request) -> Result<git::Cloned, git::Refusal> {
        let request = Request::try_from(request).map_err(|refused| {
            self.malformed(&refused);
            git::Refusal::Failed
        })?;
        if let Err(unbound) = self.sourcing.allows(&request) {
            self.malformed(&unbound);
            return Err(git::Refusal::Failed);
        }
        match self.sourcing.fetch.run_until(&request, self.sourcing.deadline) {
            Ok(cloned) => {
                let wire = git::Cloned::from(&cloned);
                self.sourcing.fetched.push(cloned);
                Ok(wire)
            }
            Err(refused) => Err(self.refuse(&refused)),
        }
    }

    fn read(&mut self, cloned: git::Cloned) -> Result<types::snapshot::Snapshot, git::Refusal> {
        let cloned = Cloned::try_from(cloned).map_err(|refused| {
            self.malformed(&refused);
            git::Refusal::Failed
        })?;
        if let Err(unbound) = self.sourcing.owns(&cloned) {
            self.malformed(&unbound);
            return Err(git::Refusal::Failed);
        }
        let read = match self.sourcing.fetch.read(&cloned, &mut self.sourcing.blobs) {
            Ok(read) => read,
            Err(refused) => return Err(self.refuse(&refused)),
        };
        let wire = types::snapshot::Snapshot::try_from(&read.snapshot).map_err(|unmirrored| {
            self.said.diagnose(Diagnostic::new(Problem::PluginValueUnmirrored { reason: unmirrored.to_string() }));
            git::Refusal::Failed
        })?;
        self.sourcing.read = Some(read);
        Ok(wire)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn state(want: &str) -> State {
        let fetch = Fetch { network: model::git::Network::Local, ..Fetch::new(std::env::temp_dir()) };
        let sourcing = Sourcing::bound(fetch, Want::new(want.parse().unwrap()), Duration::from_secs(5));
        State::new("test", Grammars::default(), &Budget::default()).sourcing(sourcing)
    }

    fn wire(locator: &str) -> git::Request {
        git::Request::from(&Request::new(locator.parse().unwrap()))
    }

    #[test]
    fn a_source_cannot_fetch_a_repository_other_than_its_want() {
        let mut state = state("file:///srv/wanted");
        let refused = git::Host::fetch(&mut state, wire("file:///srv/private"));
        assert!(matches!(refused, Err(git::Refusal::Failed)));
        assert_eq!(state.said.diagnostics.len(), 1);
        assert_eq!(state.sourcing.fetched.len(), 0);
    }

    #[test]
    fn a_plugin_with_no_want_fetches_nothing() {
        let mut state = State::new("test", Grammars::default(), &Budget::default());
        let refused = git::Host::fetch(&mut state, wire("file:///srv/wanted"));
        assert!(matches!(refused, Err(git::Refusal::Failed)));
    }

    #[test]
    fn a_source_cannot_read_a_clone_it_did_not_fetch() {
        let mut state = state("file:///srv/wanted");
        let strange = Cloned {
            clone: Hash::of(b"someone else's clone"),
            commit: Oid::parse("0".repeat(40)).unwrap(),
            shallow: false,
            objects: 1,
            bytes: model::Bytes(1),
        };
        let refused = git::Host::read(&mut state, git::Cloned::from(&strange));
        assert!(matches!(refused, Err(git::Refusal::Failed)));
        assert_eq!(state.said.diagnostics.len(), 1);
    }
}
