//! Reading a manifest back from its canonical bytes.

use std::fmt;

use super::{Header, HeaderName, HeaderValue, HeaderValueError, Manifest, MediaType, PREFIX, Served, Status};
use crate::{DuplicateRoute, Hash, Route, RouteError};

/// Bytes that are not a manifest's canonical form.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Canonical {
    /// The bytes do not start with the version prefix this code reads.
    Prefix,
    /// The bytes end inside an entry, at this offset.
    Truncated {
        /// The offset.
        at: usize,
    },
    /// A string that is not UTF-8, at this offset.
    Text {
        /// The offset.
        at: usize,
    },
    /// A route that is no route, at this offset.
    Route {
        /// The offset.
        at: usize,
        /// Why.
        source: RouteError,
    },
    /// A media type this code does not know, at this offset.
    Media {
        /// The offset.
        at: usize,
    },
    /// A status this code does not know, at this offset.
    Status {
        /// The offset.
        at: usize,
    },
    /// A header name this code does not know, at this offset.
    Header {
        /// The offset.
        at: usize,
    },
    /// A header value that is no value, at this offset.
    Value {
        /// The offset.
        at: usize,
        /// Why.
        source: HeaderValueError,
    },
    /// Two entries for one route.
    Duplicate(DuplicateRoute),
    /// Bytes after the last entry.
    Trailing {
        /// The offset.
        at: usize,
    },
    /// The entries read, but writing them again gives other bytes: the input is not canonical
    /// (entries or headers out of order).
    Altered,
}

impl fmt::Display for Canonical {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prefix => out.write_str("the manifest is not of a version this code reads"),
            Self::Truncated { at } => write!(out, "the manifest ends inside the entry at byte {at}"),
            Self::Text { at } => write!(out, "the string at byte {at} of the manifest is not UTF-8"),
            Self::Route { at, .. } => write!(out, "the route at byte {at} of the manifest is not a route"),
            Self::Media { at } => write!(out, "the media type at byte {at} of the manifest is not one this code knows"),
            Self::Status { at } => write!(out, "the status at byte {at} of the manifest is not one this code knows"),
            Self::Header { at } => {
                write!(out, "the header name at byte {at} of the manifest is not one this code knows")
            }
            Self::Value { at, .. } => write!(out, "the header value at byte {at} of the manifest is not a value"),
            Self::Duplicate(_) => out.write_str("the manifest serves a route twice"),
            Self::Trailing { at } => write!(out, "the manifest has bytes after its last entry, at byte {at}"),
            Self::Altered => out.write_str("the manifest is not in the canonical order"),
        }
    }
}

impl std::error::Error for Canonical {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Route { source, .. } => Some(source),
            Self::Value { source, .. } => Some(source),
            Self::Duplicate(source) => Some(source),
            _ => None,
        }
    }
}

/// A walk over the bytes.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Canonical> {
        let end = self.at.checked_add(count).ok_or(Canonical::Truncated { at: self.at })?;
        let taken = self.bytes.get(self.at..end).ok_or(Canonical::Truncated { at: self.at })?;
        self.at = end;
        Ok(taken)
    }

    fn number(&mut self) -> Result<u64, Canonical> {
        let at = self.at;
        let raw: [u8; 8] = self.take(8)?.try_into().map_err(|_| Canonical::Truncated { at })?;
        Ok(u64::from_le_bytes(raw))
    }

    fn string(&mut self) -> Result<&'a str, Canonical> {
        let at = self.at;
        let length = usize::try_from(self.number()?).map_err(|_| Canonical::Truncated { at })?;
        std::str::from_utf8(self.take(length)?).map_err(|_| Canonical::Text { at })
    }

    fn entry(&mut self) -> Result<Served, Canonical> {
        let at = self.at;
        let route = Route::parse(self.string()?).map_err(|source| Canonical::Route { at, source })?;
        let raw: [u8; 32] = self.take(32)?.try_into().map_err(|_| Canonical::Truncated { at })?;
        let at = self.at;
        let media = MediaType::from_essence(self.string()?).ok_or(Canonical::Media { at })?;
        let at = self.at;
        let status = Status::from_code(self.number()?).ok_or(Canonical::Status { at })?;
        let mut headers = Vec::new();
        for _ in 0..self.number()? {
            let at = self.at;
            let name = HeaderName::from_name(self.string()?).ok_or(Canonical::Header { at })?;
            let at = self.at;
            let value = HeaderValue::parse(self.string()?).map_err(|source| Canonical::Value { at, source })?;
            headers.push(Header::new(name, value));
        }
        Ok(Served::new(route, Hash::from_bytes(raw), media, headers).with_status(status))
    }
}

impl Manifest {
    /// The manifest whose [`Manifest::canonical`] bytes these are. Bytes that read but would not
    /// be written again as they are (entries or headers out of order) are [`Canonical::Altered`],
    /// so a deploy id names exactly one byte string.
    pub fn from_canonical(bytes: &[u8]) -> Result<Self, Canonical> {
        let rest = bytes.strip_prefix(PREFIX).ok_or(Canonical::Prefix)?;
        let mut cursor = Cursor { bytes: rest, at: 0 };
        let count = cursor.number()?;
        let mut entries = Vec::new();
        for _ in 0..count {
            entries.push(cursor.entry()?);
        }
        if cursor.at != rest.len() {
            return Err(Canonical::Trailing { at: cursor.at });
        }
        let manifest = Self::new(entries).map_err(Canonical::Duplicate)?;
        if manifest.canonical() != bytes {
            return Err(Canonical::Altered);
        }
        Ok(manifest)
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

    use super::*;

    fn manifest() -> Manifest {
        let csp = Header::new(HeaderName::ContentSecurityPolicy, HeaderValue::parse("default-src 'self'").unwrap());
        Manifest::new(vec![
            Served::new(Route::parse("/feed.xml").unwrap(), Hash::of(b"feed"), MediaType::Atom, vec![]),
            Served::new(Route::root(), Hash::of(b"home"), MediaType::Html, vec![csp]),
            Served::new(Route::parse("/404").unwrap(), Hash::of(b"gone"), MediaType::Html, vec![])
                .with_status(Status::NotFound),
        ])
        .unwrap()
    }

    #[test]
    fn reads_back_what_a_manifest_writes() {
        let manifest = manifest();
        let read = Manifest::from_canonical(&manifest.canonical()).unwrap();
        assert_eq!(read, manifest);
        assert_eq!(read.id(), manifest.id());
    }

    #[test]
    fn every_media_type_and_header_name_is_read_back() {
        for media in MediaType::ALL {
            assert_eq!(MediaType::from_essence(media.essence()), Some(media));
        }
        for name in HeaderName::ALL {
            assert_eq!(HeaderName::from_name(name.name()), Some(name));
        }
    }

    #[test]
    fn refuses_what_is_not_a_manifest() {
        let bytes = manifest().canonical();
        assert_eq!(Manifest::from_canonical(b"other").unwrap_err(), Canonical::Prefix);
        for cut in 0..bytes.len() {
            assert!(Manifest::from_canonical(bytes.get(..cut).unwrap()).is_err(), "a manifest cut at {cut} read");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(matches!(Manifest::from_canonical(&longer).unwrap_err(), Canonical::Trailing { .. }));
    }
}
