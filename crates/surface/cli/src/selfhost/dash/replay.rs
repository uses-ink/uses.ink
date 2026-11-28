//! A build's stored log read back for the pages that show what it said. The log is NDJSON of the
//! build's events, and the diagnostics in it have no reader (a problem's many typed fields are
//! written, not read), so each line is read as JSON, and only what can be read typed is: the
//! decisions and the abilities the source had.

use std::fmt;
use std::time::Duration;

use model::{Ability, AnyDecision, Observed};
use serde_json::Value;
use web::view::Recorded;
use web::{Cause, Failure, Part};

/// Why a stored log could not be read.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum Unreadable {
    /// A line is not JSON.
    Json {
        /// Which line, counting from 1.
        line: usize,
        /// Why.
        source: serde_json::Error,
    },
    /// A line says a decision, or what the source could do, that this version cannot read.
    Typed {
        /// Which event.
        event: &'static str,
        /// Why.
        source: serde_json::Error,
    },
}

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json { line, .. } => write!(out, "line {line} of the stored build log is not JSON"),
            Self::Typed { event, .. } => write!(out, "a `{event}` event of the stored build log cannot be read"),
        }
    }
}

impl std::error::Error for Unreadable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json { source, .. } | Self::Typed { source, .. } => Some(source),
        }
    }
}

impl From<Unreadable> for Failure {
    fn from(error: Unreadable) -> Self {
        Self::Store { part: Part::Records, source: Cause::of(&error) }
    }
}

/// The events of one stored log, in order.
#[derive(Debug)]
pub(crate) struct Replay(Vec<Value>);

impl Replay {
    /// The event whose line is `value` is called this.
    fn named(value: &Value) -> &str {
        value.get("event").and_then(Value::as_str).unwrap_or("event")
    }

    /// The event without its name: what is left to say.
    fn rest(value: &Value) -> String {
        let mut rest = value.clone();
        if let Value::Object(fields) = &mut rest {
            fields.remove("event");
        }
        rest.to_string()
    }

    /// The log `bytes` hold.
    pub(crate) fn read(bytes: &[u8]) -> Result<Self, Unreadable> {
        let mut events = Vec::new();
        for (at, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
            if !line.is_empty() {
                events.push(serde_json::from_slice(line).map_err(|source| Unreadable::Json { line: at + 1, source })?);
            }
        }
        Ok(Self(events))
    }

    /// Every event as a line of the page.
    pub(crate) fn recorded(&self) -> Vec<Recorded> {
        self.0.iter().map(Self::line).collect()
    }

    /// The events that say a problem: the diagnostics and the refusal.
    pub(crate) fn flagged(&self) -> Vec<Recorded> {
        self.0.iter().filter(|value| matches!(Self::named(value), "diagnosed" | "refused")).map(Self::line).collect()
    }

    fn line(value: &Value) -> Recorded {
        let event = Self::named(value);
        let failed = event == "finished" && value.get("status").and_then(Value::as_str) != Some("succeeded");
        Recorded { event: event.to_owned(), detail: Self::rest(value), bad: event == "refused" || failed }
    }

    /// Every decision the build recorded.
    pub(crate) fn decisions(&self) -> Result<Vec<AnyDecision>, Unreadable> {
        let mut found = Vec::new();
        for value in self.0.iter().filter(|value| Self::named(value) == "decided") {
            let mut fields = value.clone();
            if let Value::Object(map) = &mut fields {
                map.remove("event");
            }
            found
                .push(serde_json::from_value(fields).map_err(|source| Unreadable::Typed { event: "decided", source })?);
        }
        Ok(found)
    }

    /// What the source could do, as the fetch saw it. What it lacked is not read back.
    pub(crate) fn observed(&self) -> Result<Observed, Unreadable> {
        let fetched = self.0.iter().find(|value| Self::named(value) == "source-fetched");
        let has = fetched.and_then(|value| value.get("observed")).and_then(|seen| seen.get("has")).cloned();
        let has: Vec<Ability> = match has {
            Some(has) => {
                serde_json::from_value(has).map_err(|source| Unreadable::Typed { event: "source-fetched", source })?
            }
            None => Vec::new(),
        };
        Ok(Observed::new(has, Vec::new()))
    }

    /// How long the build took, as it said when it finished.
    pub(crate) fn wall(&self) -> Option<Duration> {
        let finished = self.0.iter().rev().find(|value| Self::named(value) == "finished")?;
        serde_json::from_value(finished.get("wall")?.clone()).ok()
    }
}
