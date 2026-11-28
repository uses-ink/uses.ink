//! Where sites are served.

use std::fmt;

use model::{ExternalUrl, UrlError};

use crate::{Handle, Slug};

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
    /// The name of the origin's host, without its port.
    fn name(&self) -> &str {
        match self.host.rsplit_once(':') {
            Some((name, port)) if port.bytes().all(|byte| byte.is_ascii_digit()) => name,
            _ => &self.host,
        }
    }

    /// The handle a host name is under this origin: `cstef` for `cstef.uses.ink` under
    /// `https://uses.ink`. A name with no handle in front of the origin's, or with a label that
    /// is no handle, has none. The name is the request's, in lower case and without a port.
    pub fn handle(&self, hostname: &str) -> Option<Handle> {
        let label = hostname.strip_suffix(self.name())?.strip_suffix('.')?;
        label.parse().ok()
    }

    /// The preview a host name is under this origin: `main` and `cstef` for `main--cstef.uses.ink`.
    /// A name that is a handle, or whose first label is not a label and a handle joined by `--`,
    /// is no preview, so a name is never both (decision 0091).
    pub fn preview(&self, hostname: &str) -> Option<(Slug, Handle)> {
        let label = hostname.strip_suffix(self.name())?.strip_suffix('.')?;
        let (slug, handle) = label.split_once("--")?;
        Some((slug.parse().ok()?, handle.parse().ok()?))
    }

    /// The address of `slug`'s preview of `handle`'s site.
    pub fn previewed(&self, slug: &Slug, handle: &Handle) -> Result<ExternalUrl, UrlError> {
        ExternalUrl::parse(format!("{}{}--{}.{}", self.scheme, slug, handle, self.host))
    }

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
        let local = origin("http://localhost:4000").unwrap();
        assert_eq!(local.handle("cstef.localhost"), Some(handle));
        assert_eq!(local.handle("localhost"), None);
        assert_eq!(local.handle("a.b.localhost"), None);
        assert_eq!(local.handle("cstef.example.com"), None);
        let (slug, owner) = local.preview("main--cstef.localhost").unwrap();
        assert_eq!((slug.as_str(), owner.as_str()), ("main", "cstef"));
        assert_eq!(local.handle("main--cstef.localhost"), None, "a preview host is no site");
        assert_eq!(local.preview("cstef.localhost"), None);
        assert_eq!(local.preview("a--b--cstef.localhost"), None);
        assert_eq!(local.preview("--cstef.localhost"), None);
        assert_eq!(local.preview("Main--cstef.localhost"), None);
        assert_eq!(
            origin("https://uses.ink").unwrap().previewed(&slug, &owner).unwrap().as_str(),
            "https://main--cstef.uses.ink"
        );
        assert_eq!(origin("mailto:a@b.c").unwrap_err(), OriginError::Scheme);
        assert_eq!(origin("https://uses.ink/blog").unwrap_err(), OriginError::Path);
    }
}
