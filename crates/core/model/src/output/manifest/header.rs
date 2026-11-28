//! HTTP response headers a manifest sets.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A header the engine may set. Closed: a theme cannot add arbitrary headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum HeaderName {
    /// `cache-control`.
    CacheControl,
    /// `content-security-policy`.
    ContentSecurityPolicy,
    /// `cross-origin-opener-policy`.
    CrossOriginOpenerPolicy,
    /// `cross-origin-resource-policy`.
    CrossOriginResourcePolicy,
    /// `link`.
    Link,
    /// `permissions-policy`.
    PermissionsPolicy,
    /// `referrer-policy`.
    ReferrerPolicy,
    /// `strict-transport-security`.
    StrictTransportSecurity,
    /// `x-content-type-options`.
    XContentTypeOptions,
    /// `x-frame-options`.
    XFrameOptions,
}

impl HeaderName {
    /// Every header a manifest may set.
    pub const ALL: [Self; 10] = [
        Self::CacheControl,
        Self::ContentSecurityPolicy,
        Self::CrossOriginOpenerPolicy,
        Self::CrossOriginResourcePolicy,
        Self::Link,
        Self::PermissionsPolicy,
        Self::ReferrerPolicy,
        Self::StrictTransportSecurity,
        Self::XContentTypeOptions,
        Self::XFrameOptions,
    ];

    /// The header whose [`HeaderName::name`] is `text`.
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|name| name.name() == text)
    }

    /// Lower case, as HTTP/2 writes it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::CacheControl => "cache-control",
            Self::ContentSecurityPolicy => "content-security-policy",
            Self::CrossOriginOpenerPolicy => "cross-origin-opener-policy",
            Self::CrossOriginResourcePolicy => "cross-origin-resource-policy",
            Self::Link => "link",
            Self::PermissionsPolicy => "permissions-policy",
            Self::ReferrerPolicy => "referrer-policy",
            Self::StrictTransportSecurity => "strict-transport-security",
            Self::XContentTypeOptions => "x-content-type-options",
            Self::XFrameOptions => "x-frame-options",
        }
    }
}

impl fmt::Display for HeaderName {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// A header value: visible ASCII, spaces and tabs, not starting or ending with whitespace.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct HeaderValue(String);

/// Why text is no [`HeaderValue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HeaderValueError {
    /// A byte that is not visible ASCII, space or tab, at this offset.
    Byte {
        /// The byte offset.
        at: usize,
    },
    /// Whitespace at the start or the end.
    Padded,
}

impl fmt::Display for HeaderValueError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Byte { at } => write!(out, "byte {at} of the header value is not visible ASCII"),
            Self::Padded => write!(out, "a header value does not start or end with whitespace"),
        }
    }
}

impl std::error::Error for HeaderValueError {}

impl HeaderValue {
    /// `text`, when it is visible ASCII, spaces and tabs, not padded.
    pub fn parse(text: impl Into<String>) -> Result<Self, HeaderValueError> {
        let text = text.into();
        if let Some(at) = text.bytes().position(|b| !(b.is_ascii_graphic() || b == b' ' || b == b'\t')) {
            return Err(HeaderValueError::Byte { at });
        }
        if text.trim_matches([' ', '\t']).len() != text.len() {
            return Err(HeaderValueError::Padded);
        }
        Ok(Self(text))
    }

    /// The value, as sent.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HeaderValue {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl TryFrom<String> for HeaderValue {
    type Error = HeaderValueError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(text)
    }
}

impl From<HeaderValue> for String {
    fn from(value: HeaderValue) -> Self {
        value.0
    }
}

/// One response header.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Header {
    /// Which header.
    pub name: HeaderName,
    /// Its value.
    pub value: HeaderValue,
}

impl Header {
    /// The header `name: value`.
    pub fn new(name: HeaderName, value: HeaderValue) -> Self {
        Self { name, value }
    }
}
