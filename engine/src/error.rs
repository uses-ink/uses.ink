//! The engine's own failures: rules of its own that did not hold. Each is typed until it leaves
//! the engine as a [`Diagnostic`] with a catalog code.

use std::fmt;

use model::{DecisionCode, Diagnostic, HeaderName, HeaderValueError, Problem, Route, RouteError, Subject, When};

/// A bug in the engine, or in whoever implements its traits.
#[derive(Debug)]
pub(crate) enum Broken {
    /// A decision was recorded twice for one code and subject.
    Repeated { decision: DecisionCode, subject: Subject },
    /// The assembly needed a decision no inferrer made.
    Missing { decision: DecisionCode, subject: Subject },
    /// A pin held a value of another code.
    Mismatched { decision: DecisionCode, found: DecisionCode, subject: Subject },
    /// One of the engine's header values does not parse.
    Header { name: HeaderName, source: HeaderValueError },
    /// One of the engine's routes does not parse.
    Route { written: &'static str, source: RouteError },
    /// One of the engine's paths does not parse.
    Path { written: &'static str, source: model::PathError },
    /// The engine's default language does not parse.
    Language { written: &'static str, source: model::LanguageError },
    /// Two outputs for one route.
    RouteRepeated { route: Route },
    /// The site refused parts the inferrers had checked.
    Site { source: model::SiteError },
    /// Writing XML into memory failed.
    Xml { route: Route, source: std::io::Error },
    /// A date with no instant in range.
    Date { when: When, source: jiff::Error },
}

impl fmt::Display for Broken {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repeated { decision, subject } => write!(out, "`{decision}` was decided twice for {subject}"),
            Self::Missing { decision, subject } => write!(out, "`{decision}` was never decided for {subject}"),
            Self::Mismatched { decision, found, subject } => {
                write!(out, "the pin of `{decision}` on {subject} holds a value of `{found}`")
            }
            Self::Header { name, .. } => write!(out, "the `{name}` header value does not parse"),
            Self::Route { written, .. } => write!(out, "the route `{written}` does not parse"),
            Self::Path { written, .. } => write!(out, "the path `{written}` does not parse"),
            Self::Language { written, .. } => write!(out, "the language `{written}` does not parse"),
            Self::RouteRepeated { route } => write!(out, "{route} is served twice"),
            Self::Site { .. } => write!(out, "the site refused its parts"),
            Self::Xml { route, .. } => write!(out, "{route} could not be written"),
            Self::Date { when, .. } => write!(out, "{when} has no instant"),
        }
    }
}

impl std::error::Error for Broken {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Header { source, .. } => Some(source),
            Self::Route { source, .. } => Some(source),
            Self::Path { source, .. } => Some(source),
            Self::Language { source, .. } => Some(source),
            Self::Site { source } => Some(source),
            Self::Xml { source, .. } => Some(source),
            Self::Date { source, .. } => Some(source),
            Self::Repeated { .. } | Self::Missing { .. } | Self::Mismatched { .. } | Self::RouteRepeated { .. } => None,
        }
    }
}

impl From<Broken> for Diagnostic {
    fn from(broken: Broken) -> Self {
        match broken {
            Broken::Repeated { decision, subject } => {
                Self::new(Problem::EngineDecisionRepeated { decision }).on(subject)
            }
            Broken::Missing { decision, subject } => Self::new(Problem::EngineDecisionMissing { decision }).on(subject),
            Broken::Mismatched { decision, found, subject } => {
                Self::new(Problem::EnginePinMismatched { decision, found }).on(subject)
            }
            Broken::Header { name, .. } => Self::new(Problem::EngineHeaderInvalid { header: name.name().to_owned() }),
            Broken::Route { written, .. } => Self::new(Problem::EngineRouteInvalid { written: written.to_owned() }),
            Broken::Path { written, .. } => Self::new(Problem::EnginePathInvalid { written: written.to_owned() }),
            Broken::Language { written, .. } => {
                Self::new(Problem::EngineLanguageInvalid { written: written.to_owned() })
            }
            Broken::RouteRepeated { route } => Self::new(Problem::EngineRouteRepeated { route }),
            Broken::Site { .. } => Self::new(Problem::EngineSiteRefused),
            Broken::Xml { route, .. } => Self::new(Problem::EngineXmlUnwritten { route }),
            Broken::Date { when, .. } => Self::new(Problem::EngineDateUnrepresentable { date: when.to_string() }),
        }
    }
}
