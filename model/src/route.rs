//! Slugs and the URL paths built from them.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// One URL-safe word group: lower-case Unicode letters and digits in runs joined by single `-`,
/// never starting or ending with `-`, never empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Slug(String);

/// Why text is not a [`Slug`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlugError {
    /// Nothing is left once everything but letters and digits is dropped.
    Empty,
    /// A character a slug does not hold, at this byte offset (strict parsing only).
    Character { at: usize, found: char },
    /// A `-` at the start or end, or two in a row, at this byte offset (strict parsing only).
    Hyphen { at: usize },
}

impl fmt::Display for SlugError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(out, "a slug needs at least one letter or digit"),
            Self::Character { at, found } => {
                write!(out, "`{found}` at byte {at}: a slug holds lower-case letters, digits and `-`")
            }
            Self::Hyphen { at } => write!(out, "a `-` at byte {at} starts, ends or doubles a run"),
        }
    }
}

impl std::error::Error for SlugError {}

impl Slug {
    /// The slug of any text: lower-cased, letters and digits kept, every other run turned into
    /// one `-`, trimmed of `-`. `"On Shelves, Again!"` is `on-shelves-again`.
    pub fn from_text(text: impl AsRef<str>) -> Result<Self, SlugError> {
        let mut slug = String::new();
        let mut gap = false;
        for c in text.as_ref().chars().flat_map(char::to_lowercase) {
            if c.is_alphanumeric() {
                if gap && !slug.is_empty() {
                    slug.push('-');
                }
                gap = false;
                slug.push(c);
            } else {
                gap = true;
            }
        }
        if slug.is_empty() { Err(SlugError::Empty) } else { Ok(Self(slug)) }
    }

    /// Text that already is a slug, unchanged.
    pub fn parse(text: impl Into<String>) -> Result<Self, SlugError> {
        let text = text.into();
        if text.is_empty() {
            return Err(SlugError::Empty);
        }
        let mut previous = None;
        for (at, c) in text.char_indices() {
            if c == '-' {
                if at == 0 || previous == Some('-') || at + 1 == text.len() {
                    return Err(SlugError::Hyphen { at });
                }
            } else if !is_slug_char(c) {
                return Err(SlugError::Character { at, found: c });
            }
            previous = Some(c);
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A letter or digit that lower-casing leaves alone.
fn is_slug_char(c: char) -> bool {
    c.is_alphanumeric() && c.to_lowercase().eq(std::iter::once(c))
}

impl fmt::Display for Slug {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl FromStr for Slug {
    type Err = SlugError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl TryFrom<String> for Slug {
    type Error = SlugError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(text)
    }
}

impl From<Slug> for String {
    fn from(slug: Slug) -> Self {
        slug.0
    }
}

/// A URL path on a site: `/`, or `/` then segments joined by `/`, no trailing `/`.
///
/// Each segment is one or more slugs joined by `.`, so `/posts/on-shelves` and `/feed.xml` are
/// routes and `/Posts`, `/a//b` and `/a/` are not.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Route(String);

/// Why text is not a [`Route`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RouteError {
    /// It does not start with `/`.
    Relative,
    /// It ends with `/` and is not the root.
    TrailingSlash,
    /// Segment number `index` (from zero) is not slugs joined by `.`.
    Segment { index: usize, source: SlugError },
}

impl fmt::Display for RouteError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Relative => write!(out, "a route starts with `/`"),
            Self::TrailingSlash => write!(out, "a route other than `/` does not end with `/`"),
            Self::Segment { index, .. } => write!(out, "part {} of the route is not slugs joined by `.`", index + 1),
        }
    }
}

impl std::error::Error for RouteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Segment { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl Route {
    /// `/`.
    pub fn root() -> Self {
        Self("/".to_owned())
    }

    pub fn parse(text: impl Into<String>) -> Result<Self, RouteError> {
        let text = text.into();
        let rest = text.strip_prefix('/').ok_or(RouteError::Relative)?;
        if rest.is_empty() {
            return Ok(Self(text));
        }
        if rest.ends_with('/') {
            return Err(RouteError::TrailingSlash);
        }
        for (index, segment) in rest.split('/').enumerate() {
            segment
                .split('.')
                .try_for_each(|part| Slug::parse(part).map(drop))
                .map_err(|source| RouteError::Segment { index, source })?;
        }
        Ok(Self(text))
    }

    /// The route of these slugs: none is the root.
    pub fn from_slugs<'a>(slugs: impl IntoIterator<Item = &'a Slug>) -> Self {
        let mut text = String::new();
        for slug in slugs {
            text.push('/');
            text.push_str(slug.as_str());
        }
        if text.is_empty() { Self::root() } else { Self(text) }
    }

    pub fn is_root(&self) -> bool {
        self.0 == "/"
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Its segments, none for the root.
    pub fn segments(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.0.split('/').filter(|segment| !segment.is_empty())
    }

    /// `slug` under this route.
    #[must_use]
    pub fn child(&self, slug: &Slug) -> Self {
        if self.is_root() { Self(format!("/{slug}")) } else { Self(format!("{}/{slug}", self.0)) }
    }
}

impl fmt::Display for Route {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl FromStr for Route {
    type Err = RouteError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl TryFrom<String> for Route {
    type Error = RouteError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(text)
    }
}

impl From<Route> for String {
    fn from(route: Route) -> Self {
        route.0
    }
}

#[cfg(test)]
mod test {
    use proptest::prelude::*;

    use super::{Route, RouteError, Slug, SlugError};

    #[test]
    fn slugs_any_text() {
        let slug = |text: &str| Slug::from_text(text).map(String::from);
        assert_eq!(slug("On Shelves, Again!").unwrap(), "on-shelves-again");
        assert_eq!(slug("  --Été 2024__notes--  ").unwrap(), "été-2024-notes");
        assert_eq!(slug("ÄÖ").unwrap(), "äö");
        assert_eq!(slug("日本語 ok").unwrap(), "日本語-ok");
        assert_eq!(slug("!!!"), Err(SlugError::Empty));
    }

    #[test]
    fn parses_slugs_strictly() {
        assert!(Slug::parse("a-b-2").is_ok());
        assert_eq!(Slug::parse("-a"), Err(SlugError::Hyphen { at: 0 }));
        assert_eq!(Slug::parse("a-"), Err(SlugError::Hyphen { at: 1 }));
        assert_eq!(Slug::parse("a--b"), Err(SlugError::Hyphen { at: 2 }));
        assert_eq!(Slug::parse("aB"), Err(SlugError::Character { at: 1, found: 'B' }));
        assert_eq!(Slug::parse("a b"), Err(SlugError::Character { at: 1, found: ' ' }));
        assert_eq!(Slug::parse(""), Err(SlugError::Empty));
    }

    #[test]
    fn parses_routes() {
        assert!(Route::parse("/").unwrap().is_root());
        assert_eq!(Route::parse("/posts/on-shelves").unwrap().segments().collect::<Vec<_>>(), ["posts", "on-shelves"]);
        assert!(Route::parse("/feed.xml").is_ok());
        assert_eq!(Route::parse("posts"), Err(RouteError::Relative));
        assert_eq!(Route::parse("/posts/"), Err(RouteError::TrailingSlash));
        assert!(matches!(Route::parse("/a//b"), Err(RouteError::Segment { index: 1, .. })));
        assert!(matches!(Route::parse("/Posts"), Err(RouteError::Segment { index: 0, .. })));
        assert!(matches!(Route::parse("/a/..b"), Err(RouteError::Segment { index: 1, .. })));
    }

    #[test]
    fn builds_routes_from_slugs() {
        let posts = Slug::parse("posts").unwrap();
        let hello = Slug::parse("hello").unwrap();
        assert_eq!(Route::from_slugs([]), Route::root());
        assert_eq!(Route::from_slugs([&posts, &hello]).as_str(), "/posts/hello");
        assert_eq!(Route::root().child(&posts).child(&hello).as_str(), "/posts/hello");
    }

    proptest! {
        #[test]
        fn a_slug_of_text_is_a_slug(text in "\\PC{0,32}") {
            if let Ok(slug) = Slug::from_text(&text) {
                prop_assert_eq!(Slug::parse(slug.as_str()), Ok(slug.clone()));
                prop_assert_eq!(Slug::from_text(slug.as_str()), Ok(slug));
            }
        }

        #[test]
        fn routes_of_slugs_parse_back(words in prop::collection::vec("[a-z0-9]{1,6}", 0..4)) {
            let slugs: Vec<Slug> = words.iter().map(|w| Slug::parse(w.as_str()).unwrap()).collect();
            let route = Route::from_slugs(&slugs);
            prop_assert_eq!(Route::parse(route.as_str()), Ok(route));
        }
    }
}
