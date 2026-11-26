//! The headers every entry gets, by media type (architecture.md section 13). The engine writes
//! them; no theme can change them.

use model::{Header, HeaderName, HeaderValue, MediaType};

use crate::error::Broken;

/// Reader pages run no script, load nothing from elsewhere but images and media, and cannot be
/// framed or post forms.
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
const DRAWING: &[(HeaderName, &str)] =
    &[(HeaderName::ContentSecurityPolicy, "default-src 'none'; sandbox"), (HeaderName::XContentTypeOptions, "nosniff")];

/// Every other file.
const FILE: &[(HeaderName, &str)] = &[(HeaderName::XContentTypeOptions, "nosniff")];

/// The parsed headers of each kind of entry.
#[derive(Debug)]
pub(crate) struct Headers {
    page: Vec<Header>,
    drawing: Vec<Header>,
    file: Vec<Header>,
}

impl Headers {
    pub(crate) fn new() -> Result<Self, Broken> {
        Ok(Self { page: parsed(PAGE)?, drawing: parsed(DRAWING)?, file: parsed(FILE)? })
    }

    /// The headers an entry of `media` is served with.
    pub(crate) fn of(&self, media: MediaType) -> Vec<Header> {
        match media {
            MediaType::Html => self.page.clone(),
            MediaType::Svg => self.drawing.clone(),
            _ => self.file.clone(),
        }
    }
}

fn parsed(table: &[(HeaderName, &str)]) -> Result<Vec<Header>, Broken> {
    table
        .iter()
        .map(|&(name, value)| {
            let value = HeaderValue::parse(value).map_err(|source| Broken::Header { name, source })?;
            Ok(Header::new(name, value))
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn every_value_parses() {
        let headers = Headers::new().unwrap();
        assert_eq!(headers.of(MediaType::Html).len(), PAGE.len());
        assert_eq!(headers.of(MediaType::Png).len(), 1);
    }
}
