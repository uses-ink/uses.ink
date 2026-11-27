//! A deploy: every route a build serves, the blob behind it, its media type and headers.
//!
//! # Canonical bytes
//!
//! [`Manifest::id`] is the BLAKE3 hash of these bytes, written by hand so the id never moves
//! with a serializer's version. Integers are unsigned 64-bit little-endian; a *string* is its
//! length in bytes as such an integer, then its UTF-8 bytes.
//!
//! 1. The 21 bytes `uses.ink/manifest/v2\n`.
//! 2. The number of entries, then each entry in route order:
//!    1. the route as a string;
//!    2. the blob hash, 32 raw bytes;
//!    3. the media type's [`MediaType::essence`] as a string;
//!    4. the HTTP status code as an integer;
//!    5. the number of headers, then each header, sorted by name and then value, as its lower-case name as a string
//!       and its value as a string.
//!
//! Changing any of this changes every deploy id, so it is a new version prefix, never an edit.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ContentPath, Hash, HashError, Route};

mod header;
mod read;

pub use header::{Header, HeaderName, HeaderValue, HeaderValueError};
pub use read::Canonical;

/// The bytes every manifest's canonical form starts with.
const PREFIX: &[u8] = b"uses.ink/manifest/v2\n";

/// A deploy's identity: the hash of its manifest's canonical bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeployId(Hash);

impl DeployId {
    /// The hash of the manifest's canonical bytes.
    pub const fn hash(self) -> Hash {
        self.0
    }
}

impl std::str::FromStr for DeployId {
    type Err = HashError;

    /// A deploy id as it is written: its manifest's hash in 64 lower-case hex digits.
    fn from_str(text: &str) -> Result<Self, HashError> {
        text.parse().map(Self)
    }
}

impl fmt::Display for DeployId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

/// The kind of bytes a route serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MediaType {
    /// `text/html`.
    Html,
    /// `text/css`.
    Css,
    /// `application/xml`, such as the sitemap.
    Xml,
    /// `application/atom+xml`.
    Atom,
    /// `application/rss+xml`.
    Rss,
    /// `application/json`.
    Json,
    /// `text/plain`, such as `robots.txt`.
    Text,
    /// `image/svg+xml`.
    Svg,
    /// `image/png`.
    Png,
    /// `image/jpeg`.
    Jpeg,
    /// `image/avif`.
    Avif,
    /// `image/webp`.
    Webp,
    /// `image/gif`.
    Gif,
    /// `image/x-icon`.
    Ico,
    /// `font/woff2`.
    Woff2,
    /// `application/pdf`.
    Pdf,
}

impl MediaType {
    /// Every media type, in declaration order.
    pub const ALL: [Self; 16] = [
        Self::Html,
        Self::Css,
        Self::Xml,
        Self::Atom,
        Self::Rss,
        Self::Json,
        Self::Text,
        Self::Svg,
        Self::Png,
        Self::Jpeg,
        Self::Avif,
        Self::Webp,
        Self::Gif,
        Self::Ico,
        Self::Woff2,
        Self::Pdf,
    ];

    /// The media type whose [`MediaType::essence`] is `text`.
    pub fn from_essence(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|media| media.essence() == text)
    }

    /// The type without parameters: `text/html`.
    pub const fn essence(self) -> &'static str {
        match self {
            Self::Html => "text/html",
            Self::Css => "text/css",
            Self::Xml => "application/xml",
            Self::Atom => "application/atom+xml",
            Self::Rss => "application/rss+xml",
            Self::Json => "application/json",
            Self::Text => "text/plain",
            Self::Svg => "image/svg+xml",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Avif => "image/avif",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
            Self::Ico => "image/x-icon",
            Self::Woff2 => "font/woff2",
            Self::Pdf => "application/pdf",
        }
    }

    /// Whether its bytes are text, served with `charset=utf-8`.
    pub const fn is_text(self) -> bool {
        matches!(
            self,
            Self::Html | Self::Css | Self::Xml | Self::Atom | Self::Rss | Self::Json | Self::Text | Self::Svg
        )
    }
}

impl TryFrom<&ContentPath> for MediaType {
    type Error = MediaUnknown;

    /// The media type of a file of the author's, by its extension in any case: only the images
    /// and documents a page may show or link.
    fn try_from(path: &ContentPath) -> Result<Self, MediaUnknown> {
        let unknown = || MediaUnknown { path: path.clone() };
        let extension = path.extension().ok_or_else(unknown)?.to_ascii_lowercase();
        Ok(match extension.as_str() {
            "png" => Self::Png,
            "jpg" | "jpeg" => Self::Jpeg,
            "avif" => Self::Avif,
            "webp" => Self::Webp,
            "gif" => Self::Gif,
            "svg" => Self::Svg,
            "ico" => Self::Ico,
            "pdf" => Self::Pdf,
            _ => return Err(unknown()),
        })
    }
}

/// A file of a kind uses.ink does not serve: its extension names no [`MediaType`] it knows.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MediaUnknown {
    /// The file.
    pub path: ContentPath,
}

impl fmt::Display for MediaUnknown {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} is a kind of file uses.ink does not serve", self.path)
    }
}

impl std::error::Error for MediaUnknown {}

impl fmt::Display for MediaType {
    /// The `Content-Type` value: the essence, with `; charset=utf-8` for text.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.essence())?;
        if self.is_text() { out.write_str("; charset=utf-8") } else { Ok(()) }
    }
}

/// The HTTP status a route is served with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Status {
    /// `200`.
    Ok,
    /// `404`: the site's not-found page, served for every route the manifest does not hold.
    NotFound,
}

impl Status {
    /// The status whose [`Status::code`] is `code`.
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            200 => Some(Self::Ok),
            404 => Some(Self::NotFound),
            _ => None,
        }
    }

    /// The HTTP status code: `200`, `404`.
    pub const fn code(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::NotFound => 404,
        }
    }
}

/// One file a theme serves as is, before the engine hashes it into a [`Served`] route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// Where it is served.
    pub route: Route,
    /// What it is.
    pub media: MediaType,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

/// One served route.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Served {
    route: Route,
    blob: Hash,
    media: MediaType,
    status: Status,
    headers: Vec<Header>,
}

impl Served {
    /// Served with [`Status::Ok`]. Headers are sorted by lower-case name, then value.
    pub fn new(route: Route, blob: Hash, media: MediaType, mut headers: Vec<Header>) -> Self {
        headers.sort_by(|a, b| (a.name.name(), &a.value).cmp(&(b.name.name(), &b.value)));
        Self { route, blob, media, status: Status::Ok, headers }
    }

    /// The same entry, served with `status`.
    #[must_use]
    pub fn with_status(mut self, status: Status) -> Self {
        self.status = status;
        self
    }

    /// The status it is served with.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Where it is served.
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// The hash of the bytes it serves.
    pub fn blob(&self) -> Hash {
        self.blob
    }

    /// What kind of bytes they are.
    pub fn media(&self) -> MediaType {
        self.media
    }

    /// Sorted by lower-case name, then value.
    pub fn headers(&self) -> &[Header] {
        &self.headers
    }
}

/// Two entries for one route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateRoute {
    /// The route served twice.
    pub route: Route,
}

impl fmt::Display for DuplicateRoute {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the manifest serves `{}` twice", self.route)
    }
}

impl std::error::Error for DuplicateRoute {}

/// Every route of one deploy, sorted, each once. The default serves nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct Manifest {
    entries: Vec<Served>,
}

impl Manifest {
    /// Entries are sorted by route; a route twice is an error.
    pub fn new(mut entries: Vec<Served>) -> Result<Self, DuplicateRoute> {
        entries.sort_by(|a, b| a.route.cmp(&b.route));
        for pair in entries.windows(2) {
            if let [a, b] = pair
                && a.route == b.route
            {
                return Err(DuplicateRoute { route: a.route.clone() });
            }
        }
        Ok(Self { entries })
    }

    /// Sorted by route.
    pub fn entries(&self) -> &[Served] {
        &self.entries
    }

    /// The entry served at `route`, if there is one.
    pub fn entry(&self, route: &Route) -> Option<&Served> {
        let at = self.entries.binary_search_by(|e| e.route.cmp(route)).ok()?;
        self.entries.get(at)
    }

    /// The canonical bytes described in the module documentation.
    pub fn canonical(&self) -> Vec<u8> {
        let mut bytes = PREFIX.to_vec();
        let number = |bytes: &mut Vec<u8>, n: usize| bytes.extend_from_slice(&(n as u64).to_le_bytes());
        let string = |bytes: &mut Vec<u8>, s: &str| {
            number(bytes, s.len());
            bytes.extend_from_slice(s.as_bytes());
        };
        number(&mut bytes, self.entries.len());
        for entry in &self.entries {
            string(&mut bytes, entry.route.as_str());
            bytes.extend_from_slice(entry.blob.as_bytes());
            string(&mut bytes, entry.media.essence());
            number(&mut bytes, usize::from(entry.status.code()));
            number(&mut bytes, entry.headers.len());
            for header in &entry.headers {
                string(&mut bytes, header.name.name());
                string(&mut bytes, header.value.as_str());
            }
        }
        bytes
    }

    /// The deploy's identity: the hash of [`Manifest::canonical`].
    pub fn id(&self) -> DeployId {
        DeployId(Hash::of(self.canonical()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn manifest() -> Manifest {
        let csp = Header::new(HeaderName::ContentSecurityPolicy, HeaderValue::parse("default-src 'self'").unwrap());
        Manifest::new(vec![
            Served::new(Route::parse("/feed.xml").unwrap(), Hash::of(b"feed"), MediaType::Atom, vec![]),
            Served::new(Route::root(), Hash::of(b"home"), MediaType::Html, vec![csp]),
        ])
        .unwrap()
    }

    #[test]
    fn its_id_never_moves() {
        // A change here changes every deploy id ever published: see the module docs.
        assert_eq!(manifest().id().to_string(), "f005407c617761abc69e4a13309a3b3220140ddff3d75f1d8069ae81a3405282");
    }

    #[test]
    fn sorts_and_refuses_a_route_twice() {
        let manifest = manifest();
        let routes: Vec<&str> = manifest.entries().iter().map(|e| e.route().as_str()).collect();
        assert_eq!(routes, ["/", "/feed.xml"]);
        let twice = Served::new(Route::root(), Hash::of(b""), MediaType::Html, vec![]);
        assert!(Manifest::new(vec![twice.clone(), twice]).is_err());
    }

    #[test]
    fn writes_the_documented_bytes() {
        assert_eq!(PREFIX.len(), 21);
        assert_eq!(Manifest::new(vec![]).unwrap().canonical(), [PREFIX, &0u64.to_le_bytes()].concat());
        let n = |n: u64| n.to_le_bytes().to_vec();
        let s = |s: &str| [n(s.len() as u64), s.as_bytes().to_vec()].concat();
        let expected = [
            PREFIX.to_vec(),
            n(2),
            s("/"),
            Hash::of(b"home").as_bytes().to_vec(),
            s("text/html"),
            n(200),
            n(1),
            s("content-security-policy"),
            s("default-src 'self'"),
            s("/feed.xml"),
            Hash::of(b"feed").as_bytes().to_vec(),
            s("application/atom+xml"),
            n(200),
            n(0),
        ]
        .concat();
        assert_eq!(manifest().canonical(), expected);
    }
}
