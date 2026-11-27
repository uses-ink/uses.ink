//! Interpret and render, generic over the host (docs/code.md, sections 2, 3 and 5).
//!
//! An [`Engine`] holds what the host gives it: [`Blobs`] for a snapshot's bytes, [`Pins`] for
//! `ink.toml`, [`Formats`] for reading files and a [`Theme`] for writing pages. Nothing in the
//! engine names a concrete one.
//!
//! [`Engine::interpret`] turns a [`Snapshot`](model::Snapshot) into a [`Site`](model::Site)
//! through composed stages, each recording into one journal. Every decision is a ladder of
//! witnesses: the first that testifies wins, every later one is a rejected alternative. The site
//! is assembled only from the decisions. [`Engine::render`] turns a site into a
//! [`Manifest`](model::Manifest) and the bytes behind it, through a list of artifacts: the
//! site's raster images as the host's [`Images`] prepared them, the theme's pages and files,
//! the feeds, the sitemap, `robots.txt` and the not-found page. A host keeps expensive work
//! between builds through a [`Cache`], by keys that name everything the work was made from.
//!
//! Pure: nothing here reads a clock, the environment, the filesystem or randomness.

pub mod cache;
mod error;
mod host;
pub mod image;
mod interpret;
mod render;

pub use cache::Cache;
pub use error::Error;
pub use host::{BlobMissing, Blobs, File, Format, Formats, Html, NoPins, Patterns, Pin, Pins, Record, Said, Theme};
pub use image::Images;
pub use interpret::Understood;
pub use render::{BaseUrl, BaseUrlError, Generated, RenderSettings, Rendered, Reserved};

/// The engine over one host: where bytes come from (`B`), what `ink.toml` pins (`P`), which
/// formats read files (`F`) and which theme writes pages (`T`). Interpreting needs the first
/// three, rendering the theme, so a part a call does not use can be `()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Engine<B, P, F, T> {
    /// A snapshot's bytes.
    pub blobs: B,
    /// The author's pins and patterns.
    pub pins: P,
    /// The formats, in the order they are asked.
    pub formats: F,
    /// The theme.
    pub theme: T,
}

impl<B, P, F, T> Engine<B, P, F, T> {
    /// The engine over these parts.
    pub const fn new(blobs: B, pins: P, formats: F, theme: T) -> Self {
        Self { blobs, pins, formats, theme }
    }
}
