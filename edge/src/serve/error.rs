//! Why an [`Edge`](crate::Edge) cannot be built.

use std::fmt;

use model::RouteError;

use crate::fields::FieldError;

/// A setting that makes no edge.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A fixed header, or the HSTS value, is not a header value.
    Header(FieldError),
    /// The route of the not-found page is not a route.
    Route(RouteError),
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header(_) => write!(out, "a header the edge always sends is not a header value"),
            Self::Route(_) => write!(out, "the route of the not-found page is not a route"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Header(source) => Some(source),
            Self::Route(source) => Some(source),
        }
    }
}

impl From<FieldError> for Error {
    fn from(source: FieldError) -> Self {
        Self::Header(source)
    }
}

impl From<RouteError> for Error {
    fn from(source: RouteError) -> Self {
        Self::Route(source)
    }
}
