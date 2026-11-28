//! One line of the operator log: when, how bad, from where, what happened and the facts of it.
//!
//! An [`Entry`] is typed data. The human log renders it through the terminal layer (a dim time, a
//! coloured level, a short component, the message, dimmed `key=value` facts) and the JSON log
//! serialises the same value, so the two say the same thing.

use clap::ValueEnum;
use serde::ser::SerializeMap as _;
use serde::{Serialize, Serializer};

use crate::output::term::{Cut, Doc, Glyph, Line, Render, Role};
use crate::selfhost::report::startup::Span;

use super::Happens;

/// How bad an event is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Something failed.
    Error,
    /// Something is wrong and the server goes on.
    Warn,
    /// What happened.
    Info,
}

impl Level {
    /// The level as a word, padded to the widest.
    const fn word(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn ",
            Self::Info => "info ",
        }
    }

    /// The role its word is drawn in.
    pub(crate) const fn role(self) -> Role {
        match self {
            Self::Error => Role::Bad,
            Self::Warn => Role::Warn,
            Self::Info => Role::Good,
        }
    }
}

/// Which part of the server an event is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Component {
    /// The edge answering requests.
    Http,
    /// The API answering calls.
    Api,
    /// The loop that polls git sources.
    Watch,
    /// The loop that builds what the queue holds.
    Worker,
    /// The loop that reads proof records.
    Domains,
    /// The sign-in mail sender.
    Mail,
}

impl Component {
    /// The widest name.
    const WIDTH: usize = 7;

    /// The name, as the log and the JSON write it.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Api => "api",
            Self::Watch => "watch",
            Self::Worker => "worker",
            Self::Domains => "domains",
            Self::Mail => "mail",
        }
    }
}

impl std::fmt::Display for Component {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.name())
    }
}

/// What a fact holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Datum {
    /// A name, an id or a word.
    Text(String),
    /// A number of things.
    Count(u64),
    /// A length of time in milliseconds.
    Millis(u64),
    /// A number of bytes.
    Bytes(u64),
    /// An HTTP status.
    Status(u16),
}

impl Serialize for Datum {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(text) => serializer.serialize_str(text),
            Self::Count(number) | Self::Millis(number) | Self::Bytes(number) => serializer.serialize_u64(*number),
            Self::Status(status) => serializer.serialize_u16(*status),
        }
    }
}

impl Datum {
    /// The role of a status by its class: success is good, a redirect is something to look at,
    /// a refusal warns and a failure is bad.
    const fn class(status: u16) -> Role {
        match status {
            200..=299 => Role::Good,
            300..=399 => Role::Work,
            400..=499 => Role::Warn,
            _ => Role::Bad,
        }
    }

    /// The datum as a line. A status is drawn by its class, the rest in `quiet` when asked.
    fn line(&self, quiet: bool) -> Line {
        let role = if quiet { Role::Quiet } else { Role::Plain };
        match self {
            Self::Text(text) => Line::new().with(role, text),
            Self::Count(number) => Line::new().with(role, number.to_string()),
            Self::Millis(millis) if *millis < 1000 => Line::new().with(role, format!("{millis}ms")),
            Self::Millis(millis) => Line::new().with(role, Span(std::time::Duration::from_millis(*millis)).to_string()),
            Self::Bytes(bytes) => Line::new().with(role, model::Bytes(*bytes).to_string()),
            Self::Status(status) => Line::new().with(Self::class(*status), status.to_string()),
        }
    }
}

/// One fact of an event. A bare one is written without its key, after the message: a request's
/// status and time read as `GET /tea 200 3ms`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fact {
    pub(crate) key: &'static str,
    pub(crate) datum: Datum,
    pub(crate) bare: bool,
}

impl Fact {
    /// A fact written as `key=value`.
    pub(crate) fn pair(key: &'static str, datum: Datum) -> Self {
        Self { key, datum, bare: false }
    }

    /// A fact written as its value alone.
    pub(crate) fn bare(key: &'static str, datum: Datum) -> Self {
        Self { key, datum, bare: true }
    }

    /// The fact as text, for the line that has no colour.
    pub(crate) fn text(&self) -> String {
        self.datum.line(false).text()
    }
}

/// What to do about an error, as a line to type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Remedy {
    /// A loop was given up on.
    Restart,
}

impl std::fmt::Display for Remedy {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Restart => out.write_str(
                "the loop stays stopped until `ink serve` starts again: read the cause above, or raise `--restart-limit` if the crashes were passing",
            ),
        }
    }
}

/// When an entry was written, as RFC 3339 to the second: in UTC with a `Z`, or in the local zone
/// with its offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub(crate) at: jiff::Timestamp,
    pub(crate) zone: Zone,
}

/// Which zone the time of an entry is written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Zone {
    /// UTC, written with a `Z`.
    #[default]
    Utc,
    /// The machine's own zone, written with its offset.
    Local,
}

impl std::fmt::Display for Stamp {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.zone {
            Zone::Utc => write!(out, "{}", self.at.strftime("%Y-%m-%dT%H:%M:%SZ")),
            Zone::Local => write!(
                out,
                "{}",
                self.at.to_zoned_datetime(jiff::tz::TimeZone::system()).strftime("%Y-%m-%dT%H:%M:%S%:z")
            ),
        }
    }
}

/// One event, ready to be written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) stamp: Stamp,
    pub(crate) level: Level,
    pub(crate) component: Component,
    pub(crate) name: &'static str,
    pub(crate) message: Line,
    pub(crate) facts: Vec<Fact>,
    pub(crate) causes: Vec<String>,
    pub(crate) fix: Option<Remedy>,
}

impl Entry {
    /// The facts written as `key=value`.
    fn pairs(&self) -> Line {
        let mut line = Line::new();
        for fact in self.facts.iter().filter(|fact| !fact.bare) {
            line = line.pad(1).with(Role::Quiet, fact.key).with(Role::Quiet, "=").then(fact.datum.line(true));
        }
        line
    }

    /// The facts written as their values alone, after the message.
    fn bare(&self) -> Line {
        let mut line = Line::new();
        for fact in self.facts.iter().filter(|fact| fact.bare) {
            line = line.pad(1).then(fact.datum.line(false));
        }
        line
    }
}

impl Render for Entry {
    fn render(&self, into: &mut Doc) {
        let name = self.component.name();
        let head = Line::new()
            .with(Role::Quiet, self.stamp.to_string())
            .pad(1)
            .with(self.level.role(), self.level.word())
            .pad(1)
            .strong(Role::Heading, name)
            .pad(Component::WIDTH.saturating_sub(name.len()) + 1);
        let (bare, pairs) = (self.bare(), self.pairs());
        let whole = head.clone().then(self.message.clone()).then(bare.clone()).then(pairs.clone());
        let room = into.room();
        if whole.width() <= room {
            into.push(whole);
        } else {
            // the key=value facts move under the message, and the message is cut when it still
            // does not fit: the time, the level and a request's status always stay
            let space = room.saturating_sub(head.width() + bare.width());
            let message = self.message.cut(space, Cut::End, into.look().glyphs);
            into.push(head.then(message).then(bare));
            if !pairs.is_empty() {
                into.push(Line::new().pad(1).then(pairs));
            }
        }
        for cause in &self.causes {
            let mark = into.glyph(Glyph::Last);
            into.push(Line::new().pad(2).then(mark).pad(1).with(Role::Quiet, "caused by: ").with(Role::Plain, cause));
        }
        if let Some(fix) = &self.fix {
            into.push(
                Line::new().pad(2).with(Role::Literal, "fix:").pad(1).then(Line::marked(Role::Plain, &fix.to_string())),
            );
        }
    }
}

/// The facts of an entry as a JSON object of their keys, a length or a size with its unit in the key.
struct Facts<'a>(&'a [Fact]);

impl Serialize for Facts<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for fact in self.0 {
            let key = match fact.datum {
                Datum::Millis(_) => format!("{}_ms", fact.key),
                Datum::Bytes(_) => format!("{}_bytes", fact.key),
                _ => fact.key.to_owned(),
            };
            map.serialize_entry(&key, &fact.datum)?;
        }
        map.end()
    }
}

impl Serialize for Entry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("time", &self.stamp.to_string())?;
        map.serialize_entry("level", &self.level)?;
        map.serialize_entry("component", &self.component)?;
        map.serialize_entry("event", self.name)?;
        map.serialize_entry("message", &self.message.text())?;
        map.serialize_entry("fields", &Facts(&self.facts))?;
        if !self.causes.is_empty() {
            map.serialize_entry("causes", &self.causes)?;
        }
        if let Some(fix) = &self.fix {
            map.serialize_entry("fix", &fix.to_string())?;
        }
        map.end()
    }
}

impl Entry {
    /// The entry for `event` at `stamp`.
    pub(crate) fn of(event: &dyn Happens, stamp: Stamp) -> Self {
        Self {
            stamp,
            level: event.level(),
            component: event.component(),
            name: event.name(),
            message: event.message(),
            facts: event.facts(),
            causes: event.causes(),
            fix: event.remedy(),
        }
    }

    /// An error and its causes, nearest first, as text.
    pub(crate) fn chain(error: &dyn std::error::Error) -> Vec<String> {
        std::iter::once(error.to_string()).chain(model::Chain::new(error).causes().map(ToString::to_string)).collect()
    }
}
