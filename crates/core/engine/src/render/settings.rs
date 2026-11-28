//! What rendering needs to know that no decision says: where the site is served.

use std::fmt;
use std::str::FromStr;

use model::{ExternalUrl, Route, UrlError};

/// The settings of one render. The site's title, description, authors and language are
/// decisions, so they come with the site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderSettings {
    /// Where the site is served, for the absolute links of feeds, the sitemap and `robots.txt`.
    pub base: BaseUrl,
}

/// An absolute `http` or `https` address with no query, no fragment and no trailing `/`:
/// `https://alice.uses.ink`, or `https://example.org/blog`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BaseUrl(String);

/// Why text is not a [`BaseUrl`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BaseUrlError {
    /// It is not a link out at all.
    Url(UrlError),
    /// It is a `mailto:` address.
    Scheme,
    /// It has a `?` query or a `#` fragment.
    Query,
}

impl fmt::Display for BaseUrlError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(_) => out.write_str("a site's address is an absolute URL"),
            Self::Scheme => out.write_str("a site's address starts with `https://` or `http://`"),
            Self::Query => out.write_str("a site's address has no `?` or `#`"),
        }
    }
}

impl std::error::Error for BaseUrlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Url(source) => Some(source),
            Self::Scheme | Self::Query => None,
        }
    }
}

impl FromStr for BaseUrl {
    type Err = BaseUrlError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let url = ExternalUrl::parse(text).map_err(BaseUrlError::Url)?;
        let lower = url.as_str().to_ascii_lowercase();
        if !(lower.starts_with("https://") || lower.starts_with("http://")) {
            return Err(BaseUrlError::Scheme);
        }
        if url.as_str().contains(['?', '#']) {
            return Err(BaseUrlError::Query);
        }
        Ok(Self(url.as_str().trim_end_matches('/').to_owned()))
    }
}

impl TryFrom<String> for BaseUrl {
    type Error = BaseUrlError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl BaseUrl {
    /// The absolute address of `route`: `https://alice.uses.ink/posts/a`, and the base then `/`
    /// for the root.
    pub fn at(&self, route: &Route) -> String {
        format!("{}{route}", self.0)
    }
}

impl AsRef<str> for BaseUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BaseUrl {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parses_addresses() {
        let base: BaseUrl = "https://alice.uses.ink/".parse().unwrap();
        assert_eq!(base.at(&Route::root()), "https://alice.uses.ink/");
        assert_eq!(base.at(&"/posts/a".parse().unwrap()), "https://alice.uses.ink/posts/a");
        assert_eq!("mailto:a@b.c".parse::<BaseUrl>(), Err(BaseUrlError::Scheme));
        assert_eq!("https://a.b/?x".parse::<BaseUrl>(), Err(BaseUrlError::Query));
        assert!(matches!("a.b".parse::<BaseUrl>(), Err(BaseUrlError::Url(_))));
    }
}
