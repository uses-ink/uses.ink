//! What a layer reads out of a span or an event: typed values, not text.

use std::error::Error;
use std::fmt;

use tracing::field::{Field, Visit};

/// An error as recorded: its own message, then each cause in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chain {
    /// The error's own `Display`.
    pub message: String,
    /// Each `source()`, nearest first.
    pub causes: Vec<String>,
}

impl Chain {
    /// The chain of `error`.
    pub fn of(error: &(dyn Error + 'static)) -> Self {
        let mut causes = Vec::new();
        let mut next = error.source();
        while let Some(cause) = next {
            causes.push(cause.to_string());
            next = cause.source();
        }
        Self { message: error.to_string(), causes }
    }
}

impl fmt::Display for Chain {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.message)?;
        self.causes.iter().try_for_each(|cause| write!(out, ": {cause}"))
    }
}

/// One recorded value.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// An unsigned number: a count, a size, microseconds, a status.
    Unsigned(u64),
    /// A signed number.
    Signed(i64),
    /// A float.
    Float(f64),
    /// A flag.
    Flag(bool),
    /// Text: a name, a short digest, a path, a displayed id.
    Text(String),
    /// An error and its causes.
    Error(Chain),
}

impl Value {
    /// The number, when it is an unsigned one.
    pub const fn unsigned(&self) -> Option<u64> {
        match self {
            Self::Unsigned(n) => Some(*n),
            _ => None,
        }
    }

    /// The text, when it is text.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The error chain, when it is one.
    pub const fn chain(&self) -> Option<&Chain> {
        match self {
            Self::Error(chain) => Some(chain),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsigned(n) => write!(out, "{n}"),
            Self::Signed(n) => write!(out, "{n}"),
            Self::Float(n) => write!(out, "{n}"),
            Self::Flag(flag) => write!(out, "{flag}"),
            Self::Text(text) => out.write_str(text),
            Self::Error(chain) => write!(out, "{chain}"),
        }
    }
}

/// The fields of one span or event, in the order they were written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields(Vec<(&'static str, Value)>);

impl Fields {
    /// The value of `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().rev().find(|(name, _)| *name == key).map(|(_, value)| value)
    }

    /// Every field.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Value)> {
        self.0.iter().map(|(name, value)| (*name, value))
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn put(&mut self, field: &Field, value: Value) {
        let name = field.name();
        match self.0.iter_mut().find(|(known, _)| *known == name) {
            Some(slot) => slot.1 = value,
            None => self.0.push((name, value)),
        }
    }
}

impl Visit for Fields {
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, Value::Unsigned(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, Value::Signed(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.put(field, Value::Float(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, Value::Flag(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, Value::Text(value.to_owned()));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn Error + 'static)) {
        self.put(field, Value::Error(Chain::of(value)));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.put(field, Value::Text(format!("{value:?}")));
    }
}
