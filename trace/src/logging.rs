//! `Logging`: the one value that says how a run logs, and the one place a subscriber is set up.
//!
//! The CLI builds it from the environment ([`Logging::env`]), then from its flags, and installs
//! it once. Nothing else in the workspace configures logging.

use std::collections::BTreeMap;
use std::fmt;
use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::{Layer as _, SubscriberExt as _};

use crate::filter::{Area, Filter, FilterError};
use crate::human::{Format, Human, Paint};
use crate::layer::{Logger, Sink};
use crate::lines::Lines;
use crate::timing::Timing;

/// Which variables `Logging::env` reads.
#[derive(Clone, Copy, Debug)]
pub struct Variables;

impl Variables {
    /// The filter: `ink=debug,edge=trace`.
    pub const FILTER: &'static str = "INK_LOG";
    /// A file for JSON lines, in addition to stderr.
    pub const FILE: &'static str = "INK_LOG_FILE";
    /// The layout on stderr: `pretty`, `compact` or `json`.
    pub const FORMAT: &'static str = "INK_LOG_FORMAT";
    /// Which span lines to show: `none`, `open`, `close` or `full`.
    pub const SPANS: &'static str = "INK_LOG_SPANS";
    /// Print the span timing table at the end when it is `1`.
    pub const TIMINGS: &'static str = "INK_LOG_TIMINGS";
    /// No colour when it is set and not empty.
    pub const NO_COLOR: &'static str = "NO_COLOR";
    /// No colour when it is `dumb`.
    pub const TERM: &'static str = "TERM";
}

/// Where variables come from: the process, or a map in a test.
pub trait Vars {
    /// The value of `name`, when it is set and text.
    fn var(&self, name: &str) -> Option<String>;
}

/// The process's environment.
#[derive(Clone, Copy, Debug)]
pub struct Process;

impl Vars for Process {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

impl<K: std::borrow::Borrow<str> + Ord, V: AsRef<str>> Vars for BTreeMap<K, V> {
    fn var(&self, name: &str) -> Option<String> {
        self.get(name).map(|value| value.as_ref().to_owned())
    }
}

/// A word that is none of the expected ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unknown {
    /// What was written.
    pub word: String,
    /// What would have been accepted.
    pub expected: &'static str,
}

impl fmt::Display for Unknown {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is not one of {}", self.word, self.expected)
    }
}

impl std::error::Error for Unknown {}

/// Which span lines the layers show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Spans {
    /// None: only events, still indented under the spans they are in.
    None,
    /// A line when a span opens.
    Open,
    /// A line when a span closes, with how long it took.
    Close,
    /// Both: a tree.
    #[default]
    Full,
}

impl Spans {
    pub(crate) const fn opens(self) -> bool {
        matches!(self, Self::Open | Self::Full)
    }

    pub(crate) const fn closes(self) -> bool {
        matches!(self, Self::Close | Self::Full)
    }
}

impl FromStr for Spans {
    type Err = Unknown;

    fn from_str(word: &str) -> Result<Self, Self::Err> {
        match word {
            "none" => Ok(Self::None),
            "open" => Ok(Self::Open),
            "close" => Ok(Self::Close),
            "full" => Ok(Self::Full),
            _ => Err(Unknown { word: word.to_owned(), expected: "none, open, close or full" }),
        }
    }
}

/// Whether the human layer paints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Color {
    /// When stderr is a terminal and `NO_COLOR` and `TERM=dumb` do not forbid it.
    #[default]
    Auto,
    /// Always.
    Always,
    /// Never.
    Never,
}

impl FromStr for Color {
    type Err = Unknown;

    fn from_str(word: &str) -> Result<Self, Self::Err> {
        match word {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(Unknown { word: word.to_owned(), expected: "auto, always or never" }),
        }
    }
}

/// Why logging could not be set up.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A filter did not parse.
    Filter {
        /// Where it came from: a variable or a flag.
        origin: &'static str,
        /// What was wrong.
        source: FilterError,
    },
    /// A setting is none of the expected words.
    Setting {
        /// The variable.
        origin: &'static str,
        /// What was wrong.
        source: Unknown,
    },
    /// The log file could not be opened for appending.
    File {
        /// Its path, as given.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// A subscriber is already installed in this process.
    Installed,
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filter { origin, .. } => write!(out, "the log filter in {origin} does not parse"),
            Self::Setting { origin, .. } => write!(out, "the setting {origin} is not valid"),
            Self::File { path, .. } => write!(out, "the log file {} cannot be opened", path.display()),
            Self::Installed => out.write_str("a logging subscriber is already installed in this process"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filter { source, .. } => Some(source),
            Self::Setting { source, .. } => Some(source),
            Self::File { source, .. } => Some(source),
            Self::Installed => None,
        }
    }
}

/// How a run logs. Nothing is on by default: a value from [`Logging::new`] installs nothing.
///
/// Precedence for the filter, weakest first: the level the `-v` count stands for, `INK_LOG`,
/// then `--log`. Where two name the same area the stronger one wins.
#[derive(Clone, Debug, Default)]
pub struct Logging {
    verbosity: u8,
    from_env: Filter,
    from_flag: Filter,
    file: Option<PathBuf>,
    format: Format,
    spans: Spans,
    timings: bool,
    color: Color,
    forbidden: bool,
    machine: bool,
}

impl Logging {
    /// Logging that is off.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads the variables. Call it first: a flag set afterwards wins.
    pub fn env(mut self, vars: &impl Vars) -> Result<Self, Error> {
        let setting = |origin| move |source| Error::Setting { origin, source };
        if let Some(text) = vars.var(Variables::FILTER) {
            self.from_env = text.parse().map_err(|source| Error::Filter { origin: Variables::FILTER, source })?;
        }
        self.file = vars.var(Variables::FILE).filter(|path| !path.is_empty()).map(PathBuf::from).or(self.file);
        if let Some(word) = vars.var(Variables::FORMAT) {
            self.format = word.parse().map_err(setting(Variables::FORMAT))?;
        }
        if let Some(word) = vars.var(Variables::SPANS) {
            self.spans = word.parse().map_err(setting(Variables::SPANS))?;
        }
        self.timings = vars.var(Variables::TIMINGS).is_some_and(|word| word == "1");
        self.forbidden = vars.var(Variables::NO_COLOR).is_some_and(|word| !word.is_empty())
            || vars.var(Variables::TERM).is_some_and(|word| word == "dumb");
        Ok(self)
    }

    /// The `-v` count: 1 warn, 2 info, 3 debug, 4 and more trace, for everything of ours. A healthy
    /// run writes no warning, so `-v` adds no line to one.
    #[must_use]
    pub const fn verbosity(mut self, count: u8) -> Self {
        self.verbosity = count;
        self
    }

    /// The filter of `--log`.
    #[must_use]
    pub fn filter(mut self, filter: Filter) -> Self {
        self.from_flag = filter;
        self
    }

    /// The file of `--log-file`.
    #[must_use]
    pub fn file(mut self, path: impl Into<PathBuf>) -> Self {
        self.file = Some(path.into());
        self
    }

    /// The layout on stderr.
    #[must_use]
    pub const fn format(mut self, format: Format) -> Self {
        self.format = format;
        self
    }

    /// Which span lines to show.
    #[must_use]
    pub const fn spans(mut self, spans: Spans) -> Self {
        self.spans = spans;
        self
    }

    /// Whether to print the timing table at the end.
    #[must_use]
    pub const fn timings(mut self, on: bool) -> Self {
        self.timings = on;
        self
    }

    /// Whether the human layer paints.
    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Whether stderr is a machine channel (`--json`: NDJSON events). Nothing of the log is
    /// ever written there then; only a file takes it.
    #[must_use]
    pub const fn machine(mut self, machine: bool) -> Self {
        self.machine = machine;
        self
    }

    /// The level the `-v` count stands for.
    const fn floor(&self) -> LevelFilter {
        match self.verbosity {
            0 => LevelFilter::OFF,
            1 => LevelFilter::WARN,
            2 => LevelFilter::INFO,
            3 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        }
    }

    /// Whether lines were asked for by name: a count or a filter.
    fn asked(&self) -> bool {
        self.verbosity > 0 || !self.from_env.is_empty() || !self.from_flag.is_empty()
    }

    /// The filter in force. With no count and no filter, a file or the timing table alone gets
    /// `info`: the command, build and stage spans.
    pub fn effective(&self) -> Filter {
        let base = if self.asked() { self.floor() } else { LevelFilter::INFO };
        Filter::of(Area::Ink, base).then(self.from_env.clone()).then(self.from_flag.clone())
    }

    fn paint(&self) -> Paint {
        Paint(match self.color {
            Color::Always => true,
            Color::Never => false,
            Color::Auto => !self.forbidden && std::io::stderr().is_terminal(),
        })
    }

    fn open(path: &Path) -> Result<Sink, Error> {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map(Sink::new)
            .map_err(|source| Error::File { path: path.to_path_buf(), source })
    }

    /// Installs the subscriber for the process, when anything was asked for; the guard prints
    /// the timing table when it drops.
    pub fn install(self) -> Result<Guard, Error> {
        let lines_asked = self.asked();
        let stderr = lines_asked && !self.machine;
        let file = self.file.as_deref().map(Self::open).transpose()?;
        if !stderr && file.is_none() && !self.timings {
            return Ok(Guard::inert(lines_asked && self.machine));
        }
        let human_sink = Sink::new(std::io::stderr());
        let mut human = None;
        let mut lines = Vec::new();
        if stderr && self.format == Format::Json {
            lines.push(Lines { sink: human_sink.clone(), spans: self.spans });
        } else if stderr {
            human = Some(Human { sink: human_sink.clone(), format: self.format, spans: self.spans, paint: self.paint() });
        }
        let table = file.clone().map(|sink| Lines { sink, spans: self.spans });
        lines.extend(table.clone());
        let timing = self.timings.then(Timing::default);
        let logger = Logger { start: Instant::now(), human, lines, timing: timing.clone() };
        let subscriber = Registry::default().with(logger.with_filter(self.effective().targets()));
        tracing::subscriber::set_global_default(subscriber).map_err(|_| Error::Installed)?;
        let sink = if self.machine { None } else { Some(human_sink) };
        Ok(Guard { timing, sink, file: table, withheld: false })
    }
}

/// What installing did. Dropping it prints the timing table, when it was asked for.
#[derive(Debug)]
#[must_use = "dropping the guard ends the run's log: hold it until the command is done"]
pub struct Guard {
    timing: Option<Timing>,
    sink: Option<Sink>,
    file: Option<Lines>,
    withheld: bool,
}

impl Guard {
    fn inert(withheld: bool) -> Self {
        Self { timing: None, sink: None, file: None, withheld }
    }

    /// Whether the log was asked for on a machine channel with no file to take it, so nothing was
    /// written. The CLI says so only where a human reads.
    pub const fn withheld(&self) -> bool {
        self.withheld
    }

    /// The timing summary so far, when it was asked for.
    pub fn timing(&self) -> Option<&Timing> {
        self.timing.as_ref()
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let Some(timing) = &self.timing else { return };
        if let Some(sink) = &self.sink {
            sink.put(&timing.table());
        }
        if let Some(lines) = &self.file {
            let rows: Vec<_> = timing
                .rows()
                .into_iter()
                .map(|row| {
                    serde_json::json!({
                        "path": row.path,
                        "count": row.count,
                        "total_us": u64::try_from(row.total.as_micros()).unwrap_or(u64::MAX),
                        "own_us": u64::try_from(row.own.as_micros()).unwrap_or(u64::MAX),
                    })
                })
                .collect();
            lines.sink.put(&format!("{}\n", serde_json::json!({ "timings": rows })));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn vars(pairs: &[(&'static str, &'static str)]) -> BTreeMap<&'static str, &'static str> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn nothing_is_on_by_default_and_a_bare_value_installs_nothing() {
        let guard = Logging::new().env(&vars(&[])).unwrap().install().unwrap();
        assert!(guard.timing().is_none() && !guard.withheld());
    }

    #[test]
    fn a_flag_beats_the_variable_and_the_variable_beats_the_count() {
        let logging = Logging::new()
            .env(&vars(&[("INK_LOG", "ink=info,edge=trace")]))
            .unwrap()
            .verbosity(3)
            .filter("edge=warn".parse().unwrap());
        let by_area: Vec<_> = logging.effective().directives().iter().map(|d| (d.area.to_string(), d.level)).collect();
        assert_eq!(
            by_area,
            [("ink".to_owned(), LevelFilter::INFO), ("edge".to_owned(), LevelFilter::WARN)]
        );
    }

    #[test]
    fn the_count_maps_onto_levels_and_one_v_is_a_warning_floor() {
        let level = |count| Logging::new().verbosity(count).effective().directives()[0].level;
        assert_eq!(
            [level(1), level(2), level(3), level(4)],
            [LevelFilter::WARN, LevelFilter::INFO, LevelFilter::DEBUG, LevelFilter::TRACE]
        );
    }

    #[test]
    fn a_bad_variable_is_a_typed_error_with_its_origin() {
        let bad = Logging::new().env(&vars(&[("INK_LOG", "edge=loud")])).unwrap_err();
        assert!(matches!(bad, Error::Filter { origin: "INK_LOG", .. }), "{bad:?}");
        let bad = Logging::new().env(&vars(&[("INK_LOG_FORMAT", "yaml")])).unwrap_err();
        assert!(matches!(bad, Error::Setting { origin: "INK_LOG_FORMAT", .. }), "{bad:?}");
    }

    #[test]
    fn a_machine_channel_without_a_file_withholds_the_log() {
        let guard = Logging::new().verbosity(2).machine(true).install().unwrap();
        assert!(guard.withheld());
    }

    #[test]
    fn no_color_and_a_dumb_terminal_forbid_color_in_auto() {
        let quiet = Logging::new().env(&vars(&[("NO_COLOR", "1")])).unwrap();
        assert!(!quiet.paint().0);
        assert!(Logging::new().color(Color::Always).paint().0);
    }
}
