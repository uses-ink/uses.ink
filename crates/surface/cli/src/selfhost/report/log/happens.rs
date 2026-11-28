//! What happens, as the log needs to know it: how bad, from where, what to call it, what it says
//! and the facts of it. The server's events, the loops' events and the mail sender's notes each
//! implement it, so every line of the log is built the same way.

use edge_api::Note;
use edge_axum::{Answered, Called, Cut, Event, Stopped};

use crate::output::term::{Line, Role};

use super::{Component, Datum, Entry, Fact, Level};

/// Something the operator may want a line about.
pub(crate) trait Happens {
    /// How bad it is.
    fn level(&self) -> Level;
    /// Where it is from.
    fn component(&self) -> Component;
    /// A stable name for the JSON, such as `http.answered`.
    fn name(&self) -> &'static str;
    /// What it says, as a line the log paints.
    fn message(&self) -> Line;
    /// What it holds, as typed facts.
    fn facts(&self) -> Vec<Fact> {
        Vec::new()
    }
    /// What went wrong under it, nearest cause first.
    fn causes(&self) -> Vec<String> {
        Vec::new()
    }
    /// What to do about it.
    fn remedy(&self) -> Option<super::Remedy> {
        None
    }
    /// Whether it is an ordinary request line, which is shown only when `-v` asks.
    fn request(&self) -> bool {
        false
    }
}

/// A request as the log tells it: the method and the path.
struct Request<'a> {
    method: &'a str,
    path: &'a str,
}

impl Request<'_> {
    fn line(&self) -> Line {
        Line::new().strong(Role::Plain, self.method).pad(1).with(Role::Plain, self.path)
    }
}

impl Happens for Answered {
    fn level(&self) -> Level {
        match (&self.fault, &self.refusal) {
            (Some(_), _) => Level::Error,
            (None, Some(_)) => Level::Warn,
            (None, None) if self.status.code() >= 500 => Level::Error,
            (None, None) => Level::Info,
        }
    }

    fn component(&self) -> Component {
        Component::Http
    }

    fn name(&self) -> &'static str {
        match (&self.fault, &self.refusal) {
            (Some(_), _) => "http.failed",
            (None, Some(_)) => "http.refused",
            (None, None) => "http.answered",
        }
    }

    fn message(&self) -> Line {
        Request { method: &self.method, path: &self.path }.line()
    }

    fn facts(&self) -> Vec<Fact> {
        let mut facts = vec![
            Fact::bare("status", Datum::Status(self.status.code())),
            Fact::bare("took", Datum::Millis(u64::try_from(self.took.as_millis()).unwrap_or(u64::MAX))),
            Fact::pair("kind", Datum::Text(self.kind.to_string())),
        ];
        if let Some(host) = &self.hostname {
            facts.push(Fact::pair("host", Datum::Text(host.as_str().to_owned())));
        }
        facts
    }

    fn causes(&self) -> Vec<String> {
        match (&self.fault, &self.refusal) {
            (Some(fault), _) => Entry::chain(fault),
            (None, Some(refusal)) => Entry::chain(refusal),
            (None, None) => Vec::new(),
        }
    }

    fn request(&self) -> bool {
        self.fault.is_none() && self.refusal.is_none() && self.status.code() < 500
    }
}

impl Happens for Called {
    fn level(&self) -> Level {
        if self.status >= 500 { Level::Error } else { Level::Info }
    }

    fn component(&self) -> Component {
        Component::Api
    }

    fn name(&self) -> &'static str {
        "api.answered"
    }

    fn message(&self) -> Line {
        Request { method: &self.method, path: &self.path }.line()
    }

    fn facts(&self) -> Vec<Fact> {
        vec![
            Fact::bare("status", Datum::Status(self.status)),
            Fact::bare("took", Datum::Millis(u64::try_from(self.took.as_millis()).unwrap_or(u64::MAX))),
        ]
    }

    fn request(&self) -> bool {
        self.status < 500
    }
}

impl Happens for Event {
    fn level(&self) -> Level {
        match self {
            Self::Answered(answered) => answered.level(),
            Self::Called(called) => called.level(),
            Self::Cut(_) | Self::Stopped(Stopped::Abandoned { .. }) => Level::Warn,
            Self::Connection { .. } => Level::Error,
            _ => Level::Info,
        }
    }

    fn component(&self) -> Component {
        match self {
            Self::Called(_) => Component::Api,
            _ => Component::Http,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Listening { .. } => "http.listening",
            Self::Answered(answered) => answered.name(),
            Self::Called(called) => called.name(),
            Self::Cut(_) => "http.cut",
            Self::Connection { .. } => "http.connection",
            Self::Stopping => "http.stopping",
            Self::Stopped(_) => "http.stopped",
            _ => "http.event",
        }
    }

    fn message(&self) -> Line {
        match self {
            Self::Listening { .. } => Line::plain("listening"),
            Self::Answered(answered) => answered.message(),
            Self::Called(called) => called.message(),
            Self::Cut(_) => Line::plain("response cut short"),
            Self::Connection { .. } => Line::plain("a connection failed"),
            Self::Stopping => Line::plain("stopping: no new connections"),
            Self::Stopped(Stopped::Drained) => Line::plain("stopped"),
            Self::Stopped(Stopped::Abandoned { .. }) => Line::plain("stopped with connections left open"),
            other => Line::plain(other.to_string()),
        }
    }

    fn facts(&self) -> Vec<Fact> {
        match self {
            Self::Listening { address } => vec![Fact::pair("address", Datum::Text(address.to_string()))],
            Self::Answered(answered) => answered.facts(),
            Self::Called(called) => called.facts(),
            Self::Cut(Cut { stage, limit, sent }) => vec![
                Fact::pair("stage", Datum::Text(stage.to_string())),
                Fact::pair("limit", Datum::Millis(u64::try_from(limit.as_millis()).unwrap_or(u64::MAX))),
                Fact::pair("sent", Datum::Bytes(*sent)),
            ],
            Self::Stopped(Stopped::Abandoned { limit }) => {
                vec![Fact::pair("limit", Datum::Millis(u64::try_from(limit.as_millis()).unwrap_or(u64::MAX)))]
            }
            _ => Vec::new(),
        }
    }

    fn causes(&self) -> Vec<String> {
        match self {
            Self::Answered(answered) => answered.causes(),
            Self::Connection { source } => Entry::chain(source),
            _ => Vec::new(),
        }
    }

    fn request(&self) -> bool {
        match self {
            Self::Answered(answered) => answered.request(),
            Self::Called(called) => called.request(),
            _ => false,
        }
    }
}

impl Happens for Note {
    fn level(&self) -> Level {
        match self {
            Self::Failed { .. } => Level::Error,
            Self::Dropped { .. } => Level::Warn,
            _ => Level::Info,
        }
    }

    fn component(&self) -> Component {
        Component::Mail
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Sent => "mail.sent",
            Self::Failed { .. } => "mail.failed",
            Self::Dropped { .. } => "mail.dropped",
            _ => "mail.event",
        }
    }

    fn message(&self) -> Line {
        Line::plain(self.to_string())
    }

    fn causes(&self) -> Vec<String> {
        let Self::Failed { source } = self else { return Vec::new() };
        let mut causes = Vec::new();
        let mut cause: Option<&dyn std::error::Error> = Some(source.as_ref());
        while let Some(error) = cause {
            causes.push(error.to_string());
            cause = error.source();
        }
        causes
    }
}
