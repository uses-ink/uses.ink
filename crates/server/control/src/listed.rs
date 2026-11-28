//! The routes, blobs and the manifest of a stored deploy.
//!
//! A deploy's rows say only that it exists; its manifest, the blob whose hash is the deploy's
//! id, says what it serves. Reading it back is reading the canonical bytes `model::Manifest`
//! documents, so nothing about a deploy's files is stored twice.

use model::{Hash, Manifest, Route};

pub use model::Canonical;

/// Every route of a deploy and the blob behind it, in route order, and the manifest they are
/// from, with each route's media type, status and headers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    entries: Vec<(Route, Hash)>,
    manifest: Manifest,
}

impl Listed {
    /// The routes and their blobs.
    pub fn entries(&self) -> &[(Route, Hash)] {
        &self.entries
    }

    /// The manifest the bytes held.
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

impl TryFrom<&[u8]> for Listed {
    type Error = Canonical;

    fn try_from(bytes: &[u8]) -> Result<Self, Canonical> {
        let manifest = Manifest::from_canonical(bytes)?;
        let entries = manifest.entries().iter().map(|entry| (entry.route().clone(), entry.blob())).collect();
        Ok(Self { entries, manifest })
    }
}

#[cfg(test)]
mod test {
    use model::{Header, HeaderName, HeaderValue, MediaType, Served, Status};

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
