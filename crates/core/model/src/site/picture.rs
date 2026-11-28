//! What the engine prepared from a raster image of the site, so a theme can show it at its size,
//! in the formats and widths a reader's browser picks from, over a placeholder while it loads.

use serde::Serialize;

use crate::{MediaType, Route};

/// A width and a height in pixels, as the image is shown: after its EXIF orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Dimensions {
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
}

impl Dimensions {
    /// `width` by `height` pixels.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// One copy of an image the engine serves: a format at a width.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Variant {
    /// Its format: AVIF, or the fallback every browser shows (JPEG, or PNG for an image with
    /// transparency).
    pub media: MediaType,
    /// Its size.
    pub dimensions: Dimensions,
    /// Where it is served; the route holds the hash of its bytes, so it never changes.
    pub route: Route,
}

impl Variant {
    /// `media` at `dimensions`, served at `route`.
    pub const fn new(media: MediaType, dimensions: Dimensions, route: Route) -> Self {
        Self { media, dimensions, route }
    }
}

/// A raster image as the engine prepared it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Picture {
    /// Its size as shown, for `width` and `height`, so the page does not move as it loads.
    pub dimensions: Dimensions,
    /// Its copies, by format and then width. None when the file is served as it is (an animated
    /// image, or a format the engine makes no copies of): then the asset's own route serves it.
    pub variants: Vec<Variant>,
    /// A tiny blurred image of it (its thumbhash, drawn), shown while it loads. `None` for an
    /// image with transparency, which the placeholder would show through, and for a file served
    /// as it is.
    pub placeholder: Option<Route>,
}

impl Picture {
    /// An image of `dimensions` with these copies, sorted by format and then width.
    pub fn new(dimensions: Dimensions, mut variants: Vec<Variant>, placeholder: Option<Route>) -> Self {
        variants.sort();
        Self { dimensions, variants, placeholder }
    }

    /// The copies of one format, narrowest first.
    pub fn of(&self, media: MediaType) -> impl Iterator<Item = &Variant> {
        self.variants.iter().filter(move |variant| variant.media == media)
    }
}
