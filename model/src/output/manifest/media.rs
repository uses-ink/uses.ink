//! The media type a route serves, and the table from file extension to it.

use crate::ContentPath;
use serde::{Deserialize, Serialize};
use std::fmt;

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
