//! Where sites are served.

use std::fmt;

use model::{ExternalUrl, UrlError};

use crate::Handle;

/// The address the operator serves sites under: `https://uses.ink` hosted, so the handle `cstef`
/// is served at `https://cstef.uses.ink`; `http://localhost:4000` for a trial, which gives
/// `http://cstef.localhost:4000`. A build needs it for the absolute links of feeds and sitemaps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    /// `http` or `https`, and the `://` after it.
    scheme: &'static str,
    /// The host and port, with nothing after them.
    host: String,
}

/// An address that is no origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OriginError {
    /// Not an `http` or `https` URL.
    Scheme,
    /// A path, query or fragment after the host.
    Path,
}

impl fmt::Display for OriginError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Scheme => "an origin starts with `https://` or `http://`",
            Self::Path => "an origin is a host and a port, with no path, query or fragment",
        })
    }
}

impl std::error::Error for OriginError {}

impl TryFrom<&ExternalUrl> for Origin {
    type Error = OriginError;

    fn try_from(url: &ExternalUrl) -> Result<Self, OriginError> {
        let text = url.as_str();
        let (scheme, rest) = if let Some(rest) = text.strip_prefix("https://") {
            ("https://", rest)
        } else if let Some(rest) = text.strip_prefix("http://") {
            ("http://", rest)
        } else {
            return Err(OriginError::Scheme);
        };
        let host = rest.strip_suffix('/').unwrap_or(rest);
        if host.contains(['/', '?', '#']) {
            return Err(OriginError::Path);
        }
        Ok(Self { scheme, host: host.to_owned() })
    }
}

impl Origin {
    /// The address `handle`'s site is served at.
    pub fn site(&self, handle: &Handle) -> Result<ExternalUrl, UrlError> {
        ExternalUrl::parse(format!("{}{}.{}", self.scheme, handle, self.host))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn serves_a_handle_under_the_origin() {
        let handle: Handle = "cstef".parse().unwrap();
        let origin = |text: &str| Origin::try_from(&ExternalUrl::parse(text).unwrap());
        assert_eq!(origin("https://uses.ink").unwrap().site(&handle).unwrap().as_str(), "https://cstef.uses.ink");
        assert_eq!(
            origin("http://localhost:4000/").unwrap().site(&handle).unwrap().as_str(),
            "http://cstef.localhost:4000"
        );
        assert_eq!(origin("mailto:a@b.c").unwrap_err(), OriginError::Scheme);
        assert_eq!(origin("https://uses.ink/blog").unwrap_err(), OriginError::Path);
    }
}
