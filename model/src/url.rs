//! Links out of the site.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// An `http`, `https` or `mailto` URL, as written, with no whitespace or control characters.
/// An `http(s)` URL has a host.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct ExternalUrl(String);

/// Why text is not an [`ExternalUrl`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UrlError {
    /// No scheme, or one other than `http`, `https` and `mailto`.
    Scheme,
    /// An `http(s)` URL with nothing after `//`, or a `mailto:` with no address.
    Empty,
    /// Whitespace or a control character at this byte offset.
    Character { at: usize },
}

impl fmt::Display for UrlError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheme => write!(out, "a link out starts with `https://`, `http://` or `mailto:`"),
            Self::Empty => write!(out, "the link has nothing after its scheme"),
            Self::Character { at } => write!(out, "whitespace or a control character at byte {at}"),
        }
    }
}

impl std::error::Error for UrlError {}

impl ExternalUrl {
    pub fn parse(text: impl Into<String>) -> Result<Self, UrlError> {
        let text = text.into();
        if let Some((at, _)) = text.char_indices().find(|(_, c)| c.is_whitespace() || c.is_control()) {
            return Err(UrlError::Character { at });
        }
        let (scheme, rest) = text.split_once(':').ok_or(UrlError::Scheme)?;
        let rest = match scheme.to_ascii_lowercase().as_str() {
            "http" | "https" => rest.strip_prefix("//").ok_or(UrlError::Scheme)?,
            "mailto" => rest,
            _ => return Err(UrlError::Scheme),
        };
        if rest.split(['/', '?', '#']).next().is_none_or(str::is_empty) {
            return Err(UrlError::Empty);
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExternalUrl {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl FromStr for ExternalUrl {
    type Err = UrlError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl TryFrom<String> for ExternalUrl {
    type Error = UrlError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(text)
    }
}

impl From<ExternalUrl> for String {
    fn from(url: ExternalUrl) -> Self {
        url.0
    }
}

#[cfg(test)]
mod test {
    use super::{ExternalUrl, UrlError};

    #[test]
    fn accepts_web_and_mail_links_only() {
        assert!(ExternalUrl::parse("https://example.org/a?b#c").is_ok());
        assert!(ExternalUrl::parse("HTTP://example.org").is_ok());
        assert!(ExternalUrl::parse("mailto:a@example.org").is_ok());
        assert_eq!(ExternalUrl::parse("javascript:alert(1)"), Err(UrlError::Scheme));
        assert_eq!(ExternalUrl::parse("example.org"), Err(UrlError::Scheme));
        assert_eq!(ExternalUrl::parse("https:example.org"), Err(UrlError::Scheme));
        assert_eq!(ExternalUrl::parse("https:///path"), Err(UrlError::Empty));
        assert_eq!(ExternalUrl::parse("https://a b"), Err(UrlError::Character { at: 9 }));
    }
}
