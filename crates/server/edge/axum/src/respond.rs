//! An edge answer as an axum response, and an axum request as an edge request.

use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use edge::{Field, Status};

/// A header the edge wrote that http does not take: the edge checks its values, so this is a bug
/// and the response is a `500` that says so in the event.
#[derive(Debug)]
pub enum Malformed {
    /// The name is not a valid http header name.
    Name(axum::http::header::InvalidHeaderName),
    /// The value is not a valid http header value.
    Value(axum::http::header::InvalidHeaderValue),
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "a header of the answer is not a valid http header")
    }
}

impl std::error::Error for Malformed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Name(source) => Some(source),
            Self::Value(source) => Some(source),
        }
    }
}

/// The status as http has it.
pub(crate) struct Code(pub(crate) Status);

impl From<Code> for StatusCode {
    fn from(code: Code) -> Self {
        match code.0 {
            Status::Ok => Self::OK,
            Status::Partial => Self::PARTIAL_CONTENT,
            Status::Moved => Self::MOVED_PERMANENTLY,
            Status::NotModified => Self::NOT_MODIFIED,
            Status::BadRequest => Self::BAD_REQUEST,
            Status::Forbidden => Self::FORBIDDEN,
            Status::NotFound => Self::NOT_FOUND,
            Status::MethodNotAllowed => Self::METHOD_NOT_ALLOWED,
            Status::RequestTimeout => Self::REQUEST_TIMEOUT,
            Status::UriTooLong => Self::URI_TOO_LONG,
            Status::RangeNotSatisfiable => Self::RANGE_NOT_SATISFIABLE,
            Status::HeadersTooLarge => Self::REQUEST_HEADER_FIELDS_TOO_LARGE,
            Status::Unavailable => Self::SERVICE_UNAVAILABLE,
            Status::GatewayTimeout => Self::GATEWAY_TIMEOUT,
            _ => Self::INTERNAL_SERVER_ERROR,
        }
    }
}

/// An answer's fields as http headers.
pub(crate) struct Fields<'a>(pub(crate) &'a [Field]);

impl Fields<'_> {
    /// Writes them into `response`.
    pub(crate) fn put(&self, response: &mut axum::http::Response<Body>) -> Result<(), Malformed> {
        let headers = response.headers_mut();
        for field in self.0 {
            let name = HeaderName::from_bytes(field.name.name().as_bytes()).map_err(Malformed::Name)?;
            let value = HeaderValue::from_str(field.value.as_str()).map_err(Malformed::Value)?;
            headers.append(name, value);
        }
        Ok(())
    }
}
