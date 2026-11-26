//! The engine's own failures: rules of its own, or of whoever implements its traits, that did
//! not hold. Each leaves the engine as a [`Diagnostic`] with a catalog code; its words are the
//! catalog's, so none are written here.

use std::fmt;

use model::code::AnyCode;
use model::{Diagnostic, HeaderName, HeaderValueError, LanguageError, PathError, Problem, Route, RouteError};
use model::{SiteError, Subject, When};

use crate::render::{Generated, Reserved};

/// A bug in the engine, or in a host trait's implementation.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A decision was recorded twice for one code and subject.
    Repeated {
        /// The code.
        code: AnyCode,
        /// What it was about.
        subject: Subject,
    },
    /// A decision was needed that no ladder made.
    Missing {
        /// The code.
        code: AnyCode,
        /// What it would be about.
        subject: Subject,
    },
    /// One of the engine's header values does not parse.
    Header {
        /// The header.
        name: HeaderName,
        /// Why its value does not parse.
        source: HeaderValueError,
    },
    /// One of the routes the engine serves does not parse.
    Route {
        /// The route.
        reserved: Reserved,
        /// Why it does not parse.
        source: RouteError,
    },
    /// The route of an image's copy does not parse.
    Variant {
        /// The route as written.
        written: String,
        /// Why it does not parse.
        source: RouteError,
    },
    /// The host prepared another number of images than it was asked to.
    Images {
        /// How many were asked for.
        asked: usize,
        /// How many it prepared.
        count: usize,
    },
    /// The file of a page the engine makes does not parse.
    Path {
        /// The page.
        page: Generated,
        /// Why its file does not parse.
        source: PathError,
    },
    /// The language a site is taken to be in does not parse.
    Language {
        /// The language as written.
        written: &'static str,
        /// Why it does not parse.
        source: LanguageError,
    },
    /// Two outputs for one route.
    Served {
        /// The route.
        route: Route,
    },
    /// The site refused parts the ladders had checked.
    Site {
        /// Why.
        source: SiteError,
    },
    /// Writing XML into memory failed.
    Xml {
        /// The route being written.
        route: Route,
        /// Why.
        source: std::io::Error,
    },
    /// A date with no instant in range.
    Date {
        /// The date.
        when: When,
        /// Why it has no instant.
        source: jiff::Error,
    },
}

impl Error {
    /// The catalog's problem it is.
    pub fn problem(&self) -> Problem {
        match self {
            Self::Repeated { code, .. } => Problem::EngineDecisionRepeated { decision: *code },
            Self::Missing { code, .. } => Problem::EngineDecisionMissing { decision: *code },
            Self::Header { name, .. } => Problem::EngineHeaderInvalid { header: name.name().to_owned() },
            Self::Route { reserved, .. } => Problem::EngineRouteInvalid { written: reserved.as_ref().to_owned() },
            Self::Variant { written, .. } => Problem::EngineRouteInvalid { written: written.clone() },
            Self::Images { asked, count } => {
                Problem::EngineImageMiscounted { count: *count as u64, asked: *asked as u64 }
            }
            Self::Path { page, .. } => Problem::EnginePathInvalid { written: page.as_ref().to_owned() },
            Self::Language { written, .. } => Problem::EngineLanguageInvalid { written: (*written).to_owned() },
            Self::Served { route } => Problem::EngineRouteRepeated { route: route.clone() },
            Self::Site { .. } => Problem::EngineSiteRefused,
            Self::Xml { route, .. } => Problem::EngineXmlUnwritten { route: route.clone() },
            Self::Date { when, .. } => Problem::EngineDateUnrepresentable { date: when.to_string() },
        }
    }

    /// What it is about, when it is about a decision's subject.
    pub fn subject(&self) -> Option<&Subject> {
        match self {
            Self::Repeated { subject, .. } | Self::Missing { subject, .. } => Some(subject),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    /// The catalog's message.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&diag::Message::from(&self.problem()), out)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Header { source, .. } => Some(source),
            Self::Route { source, .. } | Self::Variant { source, .. } => Some(source),
            Self::Path { source, .. } => Some(source),
            Self::Language { source, .. } => Some(source),
            Self::Site { source } => Some(source),
            Self::Xml { source, .. } => Some(source),
            Self::Date { source, .. } => Some(source),
            Self::Repeated { .. } | Self::Missing { .. } | Self::Served { .. } | Self::Images { .. } => None,
        }
    }
}

impl From<Error> for Diagnostic {
    fn from(error: Error) -> Self {
        let diagnostic = Self::new(error.problem());
        match error.subject() {
            Some(subject) => diagnostic.on(subject.clone()),
            None => diagnostic,
        }
    }
}
