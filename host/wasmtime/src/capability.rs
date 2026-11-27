//! The host's capabilities, implemented once and linked into every component (architecture.md
//! section 6): the pen through `freehand`, text measuring, highlighting through host-native's
//! [`Grammars`], and the record of what a plugin says. Each call's store holds a [`State`].

use std::fmt;
use std::time::{Duration, Instant};

use engine::{Record, Said};
use freehand::mark::{Drawn, Fill, Frame, Line, Patch, Ring, Underline};
use freehand::{Chance, Hand, Highlighter, Mark, Mulberry, Paths, Pencil, Point, Seed};
use host_native::{Failed, Fetch, Grammars, Memory, Read};
use model::Hash;
#[cfg(test)]
use model::Oid;
use model::git::{Cloned, Refused, Request, Want};
use model::highlight::Highlight;
use model::{Diagnostic, Notice, Problem};
use wasmtime::ResourceLimiter;

use crate::bind::plugin::ink::host::{diagnostic, git, highlight, pen, text};
use crate::bind::plugin::ink::model as types;
use crate::budget::Budget;

/// What one store holds: the plugin's name, the grammars, what the plugin said, and its memory
/// limit.
#[derive(Debug)]
pub(crate) struct State {
    plugin: String,
    grammars: Grammars,
    pub(crate) said: Said,
    pub(crate) limit: Limit,
    pub(crate) sourcing: Sourcing,
}

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
    pub(crate) fn new(plugin: impl Into<String>, grammars: Grammars, budget: &Budget) -> Self {
        let limit = Limit { memory: budget.bytes(), table: budget.table, refused: None };
        Self { plugin: plugin.into(), grammars, said: Said::default(), limit, sourcing: Sourcing::default() }
    }

    /// The plugin's name, as its spans and events carry it.
    pub(crate) fn plugin(&self) -> &str {
        &self.plugin
    }

    /// This state with `sourcing` as what its `git` calls use.
    pub(crate) fn sourcing(mut self, sourcing: Sourcing) -> Self {
        self.sourcing = sourcing;
        self
    }

    /// `plugin.value.malformed`: the plugin sent something the model refuses.
    fn malformed(&mut self, refused: &impl std::error::Error) {
        let problem = Problem::PluginValueMalformed { plugin: self.plugin.clone(), reason: refused.to_string() };
        self.said.diagnose(Diagnostic::new(problem));
    }
}

/// The memory and table an instance may grow to, and the first growth it was refused.
#[derive(Debug)]
pub(crate) struct Limit {
    memory: usize,
    table: u32,
    pub(crate) refused: Option<Refusal>,
}

/// A growth the limiter refused, with the size asked for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Refusal {
    Memory { wanted: usize },
    Table { wanted: usize },
}

impl ResourceLimiter for Limit {
    fn memory_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> wasmtime::Result<bool> {
        let allowed = desired <= self.memory;
        if !allowed {
            self.refused.get_or_insert(Refusal::Memory { wanted: desired });
        }
        Ok(allowed)
    }

    fn table_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> wasmtime::Result<bool> {
        let allowed = u64::try_from(desired).is_ok_and(|desired| desired <= u64::from(self.table));
        if !allowed {
            self.refused.get_or_insert(Refusal::Table { wanted: desired });
        }
        Ok(allowed)
    }
}

/// A mark drawn with whichever tool was asked for.
struct Drawing<'m, M>(&'m M);

impl<M: Mark> Drawing<'_, M> {
    fn with(&self, tool: pen::Tool, hand: &Hand, chance: &mut impl Chance) -> Paths {
        match tool {
            pen::Tool::Pencil(weight) => self.0.draw(hand, &Pencil::new(weight), chance),
            pen::Tool::Highlighter(h) => {
                self.0.draw(hand, &Highlighter { spacing: h.spacing, jitter: h.jitter }, chance)
            }
        }
    }
}

impl pen::Host for State {
    fn draw(&mut self, mark: pen::Mark, tool: pen::Tool, hand: pen::Hand, seed: String) -> Vec<String> {
        let hand = Hand::from(hand);
        let mut chance = Mulberry::from(Seed::of(&seed));
        let points = |points: Vec<pen::Point>| points.into_iter().map(Point::from).collect::<Vec<_>>();
        let paths = match mark {
            pen::Mark::Line(line) => {
                Drawing(&Line { from: line.start.into(), to: line.end.into() }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Underline(u) => Drawing(&Underline { width: u.width, y: u.y }).with(tool, &hand, &mut chance),
            pen::Mark::Frame(f) => {
                Drawing(&Frame { around: f.around.into(), slack: f.slack }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Ring(ellipse) => Drawing(&Ring(ellipse.into())).with(tool, &hand, &mut chance),
            pen::Mark::Fill(f) => {
                Drawing(&Fill { behind: f.behind.into(), shift: f.shift.into() }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Patch(corners) => Drawing(&Patch(points(corners))).with(tool, &hand, &mut chance),
            pen::Mark::Drawn(line) => Drawing(&Drawn(points(line).into_iter().collect::<freehand::Centreline>())).with(
                tool,
                &hand,
                &mut chance,
            ),
        };
        paths.iter().map(|path| path.as_str().to_owned()).collect()
    }
}

/// Measuring text needs fonts, and the host has none yet (the measurer of architecture.md
/// section 4 is not built): every font is unknown, said as the typed refusal.
impl text::Host for State {
    fn measure(&mut self, _font: String, _size: f64, _text: String) -> Result<text::Extent, text::Unmeasured> {
        Err(text::Unmeasured::FontUnknown)
    }
}

impl highlight::Host for State {
    fn highlight(&mut self, language: String, code: String) -> Result<Vec<highlight::Run>, highlight::Unhighlighted> {
        let runs =
            self.grammars.highlight(&language, &code).map_err(|refused| highlight::Unhighlighted::from(&refused))?;
        runs.into_iter().map(highlight::Run::try_from).collect::<Result<_, _>>().map_err(|unmirrored| {
            let problem = Problem::PluginValueUnmirrored { reason: unmirrored.to_string() };
            self.said.diagnose(Diagnostic::new(problem));
            highlight::Unhighlighted::Failed
        })
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

impl diagnostic::Host for State {
    fn emit(&mut self, diagnostic: types::diagnostic::Diagnostic) {
        match Diagnostic::try_from(diagnostic) {
            Ok(diagnostic) => self.said.diagnose(diagnostic),
            Err(refused) => self.malformed(&refused),
        }
    }

    fn announce(&mut self, notice: types::diagnostic::Notice) {
        match Notice::try_from(notice) {
            Ok(notice) => self.said.notice(notice),
            Err(refused) => self.malformed(&refused),
        }
    }
}

impl types::diagnostic::Host for State {}
impl types::document::Host for State {}
impl types::site::Host for State {}
impl types::read::Host for State {}
impl types::file::Host for State {}
impl types::snapshot::Host for State {}

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
