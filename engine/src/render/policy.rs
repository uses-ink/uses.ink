//! The headers every entry gets (architecture.md section 13), in one type. The engine writes
//! them; no theme can change them. `strict-transport-security` is not one of them: it belongs to
//! the host name, so the edge sends it.

use model::{Header, HeaderName, HeaderValue, MediaType};

use crate::error::Error;

/// What each kind of entry is served with: reader pages run no script and cannot be framed;
/// every other file, an author's SVG or PDF among them, is a sandboxed document no other site can
/// frame or embed, never sniffed.
#[derive(Debug)]
pub(crate) struct Policy {
    page: Vec<Header>,
    file: Vec<Header>,
}

impl Policy {
    /// Reader pages run no script, load nothing from elsewhere but images and media, and cannot
    /// be framed, post forms or be embedded by another site.
    const PAGE: &[(HeaderName, &str)] = &[
        (
            HeaderName::ContentSecurityPolicy,
            "default-src 'none'; img-src 'self' https: data:; style-src 'self'; font-src 'self'; \
             media-src 'self' https:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
        ),
        (HeaderName::CrossOriginOpenerPolicy, "same-origin"),
        (HeaderName::CrossOriginResourcePolicy, "same-origin"),
        (HeaderName::PermissionsPolicy, "camera=(), geolocation=(), microphone=(), browsing-topics=()"),
        (HeaderName::ReferrerPolicy, "strict-origin-when-cross-origin"),
        (HeaderName::XContentTypeOptions, "nosniff"),
        (HeaderName::XFrameOptions, "DENY"),
    ];

    /// Every other file is untrusted when opened on its own (an SVG or a PDF can run script):
    /// sandboxed, loading nothing, never framed, embedded only by the site itself.
    const FILE: &[(HeaderName, &str)] = &[
        (HeaderName::ContentSecurityPolicy, "default-src 'none'; frame-ancestors 'none'; sandbox"),
        (HeaderName::CrossOriginResourcePolicy, "same-origin"),
        (HeaderName::XContentTypeOptions, "nosniff"),
    ];

    /// The policy, its header values parsed.
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self { page: Self::parse(Self::PAGE)?, file: Self::parse(Self::FILE)? })
    }

    /// The headers an entry of `media` is served with.
    pub(crate) fn headers(&self, media: MediaType) -> Vec<Header> {
        match media {
            MediaType::Html => self.page.clone(),
            _ => self.file.clone(),
        }
    }

    fn parse(table: &[(HeaderName, &str)]) -> Result<Vec<Header>, Error> {
        table
            .iter()
            .map(|&(name, value)| {
                let value = HeaderValue::parse(value).map_err(|source| Error::Header { name, source })?;
                Ok(Header::new(name, value))
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn every_value_parses() {
        let policy = Policy::new().unwrap();
        assert_eq!(policy.headers(MediaType::Html).len(), Policy::PAGE.len());
        assert_eq!(policy.headers(MediaType::Png).len(), Policy::FILE.len());
    }
}
