//! The human layer: lines on stderr, spans as a tree by indentation, elapsed times, colour
//! only when it was allowed.

use std::fmt::Write as _;
use std::str::FromStr;

use tracing::Level;

use crate::layer::{Entry, Sink, What};
use crate::record::{Fields, Value};
use crate::{Spans, Unknown};

/// How the human layer lays a line out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Format {
    /// A tree: spans open and close as lines, and everything inside is indented.
    #[default]
    Pretty,
    /// One line per event with the span path in front, no span lines: easy to `grep`.
    Compact,
    /// JSON lines, for machines.
    Json,
}

impl FromStr for Format {
    type Err = Unknown;

    fn from_str(word: &str) -> Result<Self, Self::Err> {
        match word {
            "pretty" => Ok(Self::Pretty),
            "compact" => Ok(Self::Compact),
            "json" => Ok(Self::Json),
            _ => Err(Unknown { word: word.to_owned(), expected: "pretty, compact or json" }),
        }
    }
}

/// The colour roles of the log: the level, and faint for what is not the point. The same
/// eight ANSI colours the rest of `ink` uses, so the terminal's theme decides the hue.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Paint(pub(crate) bool);

impl Paint {
    fn level(self, level: Level) -> String {
        let text = format!("{level:<5}");
        if !self.0 {
            return text;
        }
        let code = match level {
            Level::ERROR => "1;31",
            Level::WARN => "33",
            Level::INFO => "32",
            Level::DEBUG => "34",
            Level::TRACE => "35",
        };
        format!("\x1b[{code}m{text}\x1b[0m")
    }

    fn faint(self, text: &str) -> String {
        if self.0 { format!("\x1b[2m{text}\x1b[0m") } else { text.to_owned() }
    }
}

/// The human layer's settings and where it writes.
#[derive(Clone, Debug)]
pub(crate) struct Human {
    pub(crate) sink: Sink,
    pub(crate) format: Format,
    pub(crate) spans: Spans,
    pub(crate) paint: Paint,
}

impl Human {
    fn value(value: &Value) -> String {
        match value {
            Value::Text(text) if !text.is_empty() && !text.contains([' ', '"', '=', '\n']) => text.clone(),
            Value::Text(text) => format!("{text:?}"),
            Value::Error(chain) => format!("{:?}", chain.message),
            other => other.to_string(),
        }
    }

    fn fields(fields: &Fields, skip: &[&str]) -> String {
        let mut text = String::new();
        for (key, value) in fields.iter().filter(|(key, _)| !skip.contains(key)) {
            let _written = write!(text, " {key}={}", Self::value(value));
        }
        text
    }

    /// One `caused by` line per cause of every error field, under the line that holds it.
    fn causes(&self, fields: &Fields, indent: &str, out: &mut String) {
        for (_, value) in fields.iter() {
            if let Value::Error(chain) = value {
                for cause in &chain.causes {
                    let _written = writeln!(out, "{indent}        {} {cause}", self.paint.faint("caused by:"));
                }
            }
        }
    }

    pub(crate) fn write(&self, entry: &Entry<'_>) {
        let tree = self.format == Format::Pretty;
        let depth = match entry.what {
            What::Event { .. } => entry.frames.len(),
            _ => entry.frames.len().saturating_sub(1),
        };
        let indent = if tree { "  ".repeat(depth) } else { String::new() };
        let body = match &entry.what {
            What::Open if tree && self.spans.opens() => {
                let label = entry.frames.last().map_or_default(crate::layer::Frame::label);
                format!("> {label}{}", Self::fields(entry.fields, &["kind"]))
            }
            What::Close { elapsed, own } if tree && self.spans.closes() => {
                let label = entry.frames.last().map_or_default(crate::layer::Frame::label);
                let own = if own == elapsed { String::new() } else { format!(", own {own:.1?}") };
                format!("< {label} {}{}", self.paint.faint(&format!("{elapsed:.1?}{own}")), Self::fields(entry.fields, &["kind"]))
            }
            What::Event { name } => {
                let shown = if name.starts_with("event ") {
                    entry.fields.get("message").map_or_default(Value::to_string)
                } else {
                    format!("{name}{}", Self::fields(entry.fields, &[]))
                };
                if tree {
                    shown
                } else {
                    let path: Vec<_> = entry.frames.iter().map(crate::layer::Frame::label).collect();
                    format!("[{}] {shown}", path.join(">"))
                }
            }
            _ => return,
        };
        let at = self.paint.faint(&format!("{:>8.3}s", entry.at.as_secs_f64()));
        let mut line = format!("{at} {} {indent}{body}\n", self.paint.level(entry.level));
        self.causes(entry.fields, &indent, &mut line);
        self.sink.put(&line);
    }
}
