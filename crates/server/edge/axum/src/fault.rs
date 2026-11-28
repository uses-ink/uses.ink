//! The failures an [`Event`](crate::Event) carries, as data with their sources.

use std::error::Error;
use std::fmt;

use crate::respond::Malformed;

/// An error that cannot cross a thread as it is (its type holds a port's error) kept as what
/// each error in its chain said, outermost first. It is an error itself, with the same chain of
/// sources, so a caller that walks `source()` sees what the original said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cause {
    said: String,
    below: Option<Box<Cause>>,
}

impl Cause {
    /// `error` and every cause below it.
    pub fn of(error: &(dyn Error + 'static)) -> Self {
        Self { said: error.to_string(), below: error.source().map(|below| Box::new(Self::of(below))) }
    }
}

impl fmt::Display for Cause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.said)
    }
}

impl Error for Cause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.below.as_deref().map(|below| below as &(dyn Error + 'static))
    }
}

/// What failed on one request.
#[derive(Debug)]
#[non_exhaustive]
pub enum AnswerFault {
    /// The edge could not answer: a lookup or a blob failed, or a file is over the limit.
    Edge(Cause),
    /// The edge wrote a header that http does not take.
    Header(Malformed),
}

impl fmt::Display for AnswerFault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Edge(_) => out.write_str("the edge could not answer"),
            Self::Header(_) => out.write_str("the answer has a header that http does not take"),
        }
    }
}

impl Error for AnswerFault {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Edge(source) => Some(source),
            Self::Header(source) => Some(source),
        }
    }
}

/// A connection or an accept that failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum ConnectionFault {
    /// The accepted socket could not be set up for lingering.
    Linger(std::io::Error),
    /// Hyper ended the connection with an error.
    Serve(hyper::Error),
    /// The listener could not accept.
    Accept(std::io::Error),
}

impl fmt::Display for ConnectionFault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linger(_) => out.write_str("a connection could not be set up to linger"),
            Self::Serve(_) => out.write_str("a connection ended with an error"),
            Self::Accept(_) => out.write_str("the listener could not accept a connection"),
        }
    }
}

impl Error for ConnectionFault {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Linger(source) | Self::Accept(source) => Some(source),
            Self::Serve(source) => Some(source),
        }
    }
}
