//! The routes and blobs of a stored manifest.
//!
//! A deploy's rows say only that it exists; its manifest, the blob whose hash is the deploy's
//! id, says what it serves. Reading it back is reading the canonical bytes `model::Manifest`
//! documents, so nothing about a deploy's files is stored twice.

use std::fmt;

use model::{Hash, Route, RouteError};

/// What the canonical bytes start with, as `model::Manifest` writes them.
const PREFIX: &[u8] = b"uses.ink/manifest/v2\n";

/// Every route of a deploy and the blob behind it, in route order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed(Vec<(Route, Hash)>);

impl Listed {
    /// The routes and their blobs.
    pub fn entries(&self) -> &[(Route, Hash)] {
        &self.0
    }
}

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
    /// Bytes after the last entry.
    Trailing {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for Canonical {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prefix => out.write_str("the manifest is not of a version this code reads"),
            Self::Truncated { at } => write!(out, "the manifest ends inside the entry at byte {at}"),
            Self::Text { at } => write!(out, "the string at byte {at} of the manifest is not UTF-8"),
            Self::Route { at, .. } => write!(out, "the route at byte {at} of the manifest is not a route"),
            Self::Trailing { at } => write!(out, "the manifest has bytes after its last entry, at byte {at}"),
        }
    }
}

impl std::error::Error for Canonical {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Route { source, .. } => Some(source),
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
}

impl TryFrom<&[u8]> for Listed {
    type Error = Canonical;

    fn try_from(bytes: &[u8]) -> Result<Self, Canonical> {
        let rest = bytes.strip_prefix(PREFIX).ok_or(Canonical::Prefix)?;
        let mut cursor = Cursor { bytes: rest, at: 0 };
        let count = cursor.number()?;
        let mut entries = Vec::new();
        for _ in 0..count {
            let at = cursor.at;
            let route = Route::parse(cursor.string()?).map_err(|source| Canonical::Route { at, source })?;
            let raw: [u8; 32] = cursor.take(32)?.try_into().map_err(|_| Canonical::Truncated { at })?;
            cursor.string()?;
            cursor.number()?;
            for _ in 0..cursor.number()? {
                cursor.string()?;
                cursor.string()?;
            }
            entries.push((route, Hash::from_bytes(raw)));
        }
        if cursor.at != rest.len() {
            return Err(Canonical::Trailing { at: cursor.at });
        }
        Ok(Self(entries))
    }
}

#[cfg(test)]
mod test {
    use model::{Header, HeaderName, HeaderValue, Manifest, MediaType, Served, Status};

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
        let listed = Listed::try_from(manifest.canonical().as_slice()).unwrap();
        let expected: Vec<(Route, Hash)> = manifest.entries().iter().map(|e| (e.route().clone(), e.blob())).collect();
        assert_eq!(listed.entries(), expected);
    }

    #[test]
    fn refuses_what_is_not_a_manifest() {
        let bytes = manifest().canonical();
        assert_eq!(Listed::try_from(&b"other"[..]).unwrap_err(), Canonical::Prefix);
        for cut in 0..bytes.len() {
            assert!(Listed::try_from(bytes.get(..cut).unwrap()).is_err(), "a manifest cut at {cut} read");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(matches!(Listed::try_from(longer.as_slice()).unwrap_err(), Canonical::Trailing { .. }));
    }
}
