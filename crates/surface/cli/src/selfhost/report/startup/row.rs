//! One row of the start report: a key, a typed value, where the value came from and what it
//! warns about.
//!
//! A value is a number, a size, a length of time, an address or a name, never a sentence, so the
//! human report paints it by what it is and the JSON report carries it with its unit.

use std::path::Path;
use std::time::Duration;

use clap::ValueEnum;
use serde::ser::SerializeStruct as _;
use serde::{Serialize, Serializer};

use crate::output::term::{Line, Role};

/// A length of time, written in the units a person reads: `10m`, `1h 30m`, `250ms`, `0s`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span(pub(crate) Duration);

impl Span {
    /// The units, largest first, and the seconds each holds.
    const UNITS: [(&'static str, u64); 4] = [("d", 86_400), ("h", 3_600), ("m", 60), ("s", 1)];

    /// The parts a length is written in, largest first, with the parts that are zero left out. A
    /// length under a second is in milliseconds and none at all is `0s`.
    pub(crate) fn parts(self) -> Vec<(u64, &'static str)> {
        let mut left = self.0.as_secs();
        if left == 0 {
            let millis = u64::from(self.0.subsec_millis());
            return vec![if millis == 0 { (0, "s") } else { (millis, "ms") }];
        }
        let mut parts = Vec::new();
        for (unit, size) in Self::UNITS {
            if left >= size {
                parts.push((left / size, unit));
                left %= size;
            }
        }
        parts
    }

    /// The length as a line: each number in the figure role and each unit quiet.
    pub(crate) fn line(self) -> Line {
        let mut line = Line::new();
        for (at, (number, unit)) in self.parts().into_iter().enumerate() {
            if at > 0 {
                line = line.pad(1);
            }
            line = line.with(Role::Figure, number.to_string()).with(Role::Quiet, unit);
        }
        line
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (at, (number, unit)) in self.parts().into_iter().enumerate() {
            if at > 0 {
                out.write_str(" ")?;
            }
            write!(out, "{number}{unit}")?;
        }
        Ok(())
    }
}

impl Serialize for Span {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0.as_secs())
    }
}

/// What a row holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "unit", content = "amount", rename_all = "snake_case")]
pub(crate) enum Value {
    /// An address to open.
    Url(String),
    /// A place on disk.
    Path(String),
    /// Something to type.
    Command(String),
    /// A word or a name.
    Text(String),
    /// A number of things.
    Count(u64),
    /// A number of bytes, written in binary units.
    Bytes(u64),
    /// A length of time, in seconds in the JSON.
    Seconds(Span),
    /// At most `count` in each `per`.
    Rate { count: u32, per: Span },
    /// Yes or no.
    Flag(bool),
    /// Set and never printed.
    Secret,
    /// Several values.
    List(Vec<Value>),
}

impl Value {
    /// A place on disk.
    pub(crate) fn path(path: &Path) -> Self {
        Self::Path(path.display().to_string())
    }

    /// The name a flag's value is written with.
    pub(crate) fn named<T: ValueEnum>(value: &T) -> Self {
        Self::Text(value.to_possible_value().map_or_default(|possible| possible.get_name().to_owned()))
    }

    /// A size as `256 MiB`: the number in the figure role, the unit quiet.
    fn measured(bytes: u64) -> Line {
        let text = model::Bytes(bytes).to_string();
        match text.split_once(' ') {
            Some((number, unit)) => Line::new().with(Role::Figure, number).pad(1).with(Role::Quiet, unit),
            None => Line::new().with(Role::Figure, text),
        }
    }

    /// The value as a line, painted by what it is.
    pub(crate) fn line(&self) -> Line {
        match self {
            Self::Url(url) => Line::new().with(Role::Link, url),
            Self::Path(path) => Line::new().with(Role::Place, path),
            Self::Command(command) => Line::new().with(Role::Literal, command),
            Self::Text(text) => Line::plain(text),
            Self::Count(count) => Line::new().with(Role::Figure, count.to_string()),
            Self::Bytes(bytes) => Self::measured(*bytes),
            Self::Seconds(span) => span.line(),
            Self::Rate { count, per } => {
                Line::new().with(Role::Figure, count.to_string()).with(Role::Quiet, " per ").then(per.line())
            }
            Self::Flag(true) => Line::new().with(Role::Good, "yes"),
            Self::Flag(false) => Line::new().with(Role::Quiet, "no"),
            Self::Secret => Line::new().with(Role::Quiet, "set, never printed"),
            Self::List(items) => {
                let mut line = Line::new();
                for (at, item) in items.iter().enumerate() {
                    if at > 0 {
                        line = line.with(Role::Quiet, ", ");
                    }
                    line = line.then(item.line());
                }
                line
            }
        }
    }
}

/// Where a value came from, so a person knows whether to change it and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "flag", rename_all = "snake_case")]
pub(crate) enum Source {
    /// Nothing was asked: this is the default.
    Default,
    /// This flag or its environment variable changed the default.
    Flag(&'static str),
    /// Worked out from other settings; this flag overrides it.
    Derived(&'static str),
    /// Worked out, and no flag sets it.
    Computed,
    /// Off until this flag turns it on.
    Needs(&'static str),
}

impl Source {
    /// The flag when `given`, else the default.
    pub(crate) const fn given(given: bool, flag: &'static str) -> Self {
        if given { Self::Flag(flag) } else { Self::Default }
    }

    /// The flag when `value` is not the flag's default.
    pub(crate) fn changed<T: ValueEnum + Default>(value: &T, flag: &'static str) -> Self {
        let name = |value: &T| value.to_possible_value().map(|possible| possible.get_name().to_owned());
        Self::given(name(value) != name(&T::default()), flag)
    }

    /// The source as a line: quiet for a default, the flag in its own role when it changed one.
    pub(crate) fn line(self) -> Line {
        match self {
            Self::Default => Line::new().with(Role::Quiet, "default"),
            Self::Flag(flag) => Line::new().with(Role::Literal, flag),
            Self::Derived(flag) => {
                Line::new().with(Role::Quiet, "derived (").with(Role::Literal, flag).with(Role::Quiet, ")")
            }
            Self::Computed => Line::new().with(Role::Quiet, "computed"),
            Self::Needs(flag) => Line::new().with(Role::Quiet, "turn on with ").with(Role::Literal, flag),
        }
    }
}

/// Something a row warns about, with what to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Caution {
    /// The origin is plain http, so the session cookie is not `Secure`.
    PlainHttp,
    /// A token file of an older version is still in the data folder and no longer works.
    Retired { file: String },
}

impl Caution {
    /// A stable name for the JSON.
    const fn kind(&self) -> &'static str {
        match self {
            Self::PlainHttp => "cookie.plain_http",
            Self::Retired { .. } => "owner_token.retired",
        }
    }

    /// What is wrong.
    pub(crate) fn message(&self) -> String {
        match self {
            Self::PlainHttp => "plain http: the session cookie is not Secure".to_owned(),
            Self::Retired { file } => {
                let note = model::Announcement::AuthTokenRetired { token_file: file.clone() };
                diag::Message::from(&note).to_string()
            }
        }
    }

    /// What to do about it.
    pub(crate) fn fix(&self) -> String {
        match self {
            Self::PlainHttp => {
                "end TLS in a proxy and give `--origin https://<your name>`; plain http is for this machine only"
                    .to_owned()
            }
            Self::Retired { file } => {
                let note = model::Announcement::AuthTokenRetired { token_file: file.clone() };
                diag::Fix::of_announcement(&note).map_or_else(String::new, |fix| fix.to_string())
            }
        }
    }
}

impl Serialize for Caution {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Caution", 3)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.message())?;
        state.serialize_field("fix", &self.fix())?;
        state.end()
    }
}

/// One setting: its key, value and source, and a caution when it needs one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Row {
    pub(crate) key: &'static str,
    pub(crate) value: Value,
    pub(crate) source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) caution: Option<Caution>,
}

impl Row {
    /// A row with no caution.
    pub(crate) const fn new(key: &'static str, value: Value, source: Source) -> Self {
        Self { key, value, source, caution: None }
    }

    /// The same row with a caution.
    #[must_use]
    pub(crate) fn warned(mut self, caution: Caution) -> Self {
        self.caution = Some(caution);
        self
    }

    /// The key as a person reads it: `nonces_per_key` is `nonces per key`.
    pub(crate) fn label(&self) -> String {
        self.key.replace('_', " ")
    }
}
