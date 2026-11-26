//! Interpret and render.
//!
//! [`interpret`] turns a [`Snapshot`](model::Snapshot) into a [`Site`](model::Site) through an
//! ordered list of inferrers (architecture.md section 9). Each inferrer records
//! [`Decision`](model::Decision)s with their evidence and the alternatives they beat, and the
//! site is assembled only from those decisions. [`render`] turns a site into a
//! [`Manifest`](model::Manifest) and the bytes behind it, through a [`Theme`].
//!
//! Pure: nothing here reads a clock, the environment, the filesystem or randomness. Bytes come
//! through [`Blobs`], configuration through [`Pins`], file formats through [`Format`].

mod error;
mod interpret;
mod pins;
mod render;

use std::borrow::Cow;
use std::fmt;

pub use interpret::{Understood, explain, interpret};
pub use pins::{NoPins, Pin, Pins};
pub use render::{BaseUrl, BaseUrlError, RenderSettings, Rendered, render};

/// Where the engine reads a snapshot's bytes from.
pub trait Blobs {
    /// The bytes whose hash is `hash`.
    fn read(&self, hash: &model::Hash) -> Result<Cow<'_, [u8]>, BlobMissing>;
}

/// A blob the store does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobMissing {
    /// The hash asked for.
    pub hash: model::Hash,
}

impl fmt::Display for BlobMissing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the blob store holds no blob {}", self.hash)
    }
}

impl std::error::Error for BlobMissing {}

/// A format plugin: reads one file's bytes into a document and its frontmatter.
pub trait Format {
    /// Whether this format reads the file at `path`.
    fn handles(&self, path: &model::ContentPath) -> bool;
    /// The file at `path`, read. It resolves nothing: links to other files stay as written.
    fn read(&self, path: &model::ContentPath, bytes: &[u8]) -> model::Read;
}

/// A theme plugin: renders each page, and gives the files every page shares.
pub trait Theme {
    /// Its name, as diagnostics show it.
    fn name(&self) -> &str;
    /// CSS, fonts and images, asked for once per build.
    fn assets(&self) -> Vec<File>;
    /// One page's HTML. `site` is the whole site, for navigation and links.
    #[expect(clippy::result_large_err, reason = "the theme contract returns the model's diagnostic as is (0014)")]
    fn render(&self, page: &model::Page, site: &model::Site) -> Result<Html, model::Diagnostic>;
}

/// One file a theme serves as is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// Where it is served.
    pub route: model::Route,
    /// What it is.
    pub media: model::MediaType,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

/// A page as a theme rendered it. The engine adds nothing inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Html(String);

impl From<String> for Html {
    fn from(html: String) -> Self {
        Self(html)
    }
}

impl AsRef<str> for Html {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Html> for Vec<u8> {
    fn from(html: Html) -> Self {
        html.0.into_bytes()
    }
}
