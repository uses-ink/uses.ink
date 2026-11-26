//! The headers every entry gets (architecture.md section 13), in one type. The engine writes
//! them; no theme can change them.

use model::{Header, HeaderName, HeaderValue, MediaType};

use crate::error::Error;

/// What each kind of entry is served with: reader pages run no script and cannot be framed, an
/// author's SVG is a sandboxed file, every other file is never sniffed.
#[derive(Debug)]
pub(crate) struct Policy {
    page: Vec<Header>,
    drawing: Vec<Header>,
    file: Vec<Header>,
}

impl Policy {
    /// Reader pages run no script, load nothing from elsewhere but images and media, and cannot
    /// be framed or post forms.
    const PAGE: &[(HeaderName, &str)] = &[
        (
            HeaderName::ContentSecurityPolicy,
            "default-src 'none'; img-src 'self' https: data:; style-src 'self'; font-src 'self'; \
             media-src 'self' https:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
        ),
        (HeaderName::CrossOriginOpenerPolicy, "same-origin"),
        (HeaderName::PermissionsPolicy, "camera=(), geolocation=(), microphone=(), browsing-topics=()"),
        (HeaderName::ReferrerPolicy, "strict-origin-when-cross-origin"),
        (HeaderName::XContentTypeOptions, "nosniff"),
        (HeaderName::XFrameOptions, "DENY"),
    ];

    /// An SVG from the author's source is untrusted: served only as a sandboxed file.
    const DRAWING: &[(HeaderName, &str)] = &[
        (HeaderName::ContentSecurityPolicy, "default-src 'none'; sandbox"),
        (HeaderName::XContentTypeOptions, "nosniff"),
    ];

    /// Every other file.
    const FILE: &[(HeaderName, &str)] = &[(HeaderName::XContentTypeOptions, "nosniff")];

    /// The policy, its header values parsed.
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self {
            page: Self::parse(Self::PAGE)?,
            drawing: Self::parse(Self::DRAWING)?,
            file: Self::parse(Self::FILE)?,
        })
    }

    /// The headers an entry of `media` is served with.
    pub(crate) fn headers(&self, media: MediaType) -> Vec<Header> {
        match media {
            MediaType::Html => self.page.clone(),
            MediaType::Svg => self.drawing.clone(),
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
        assert_eq!(policy.headers(MediaType::Png).len(), 1);
    }
}
