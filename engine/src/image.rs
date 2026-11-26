//! The host's image work. The engine owns image variants (decision 0014): which files get them,
//! their routes and headers, and what a theme learns of them. Making them (decoding within a
//! budget, orienting, resizing, encoding, drawing a placeholder) is pixel work the host does
//! natively, as with highlighting (decision 0039), so the engine stays small and portable.

use model::{ContentPath, Diagnostic, Dimensions, Hash, MediaType};

/// What prepares a site's raster images: the host's image capability.
pub trait Images {
    /// Why an image could not be prepared, said as a diagnostic about its file.
    type Refusal: Into<Diagnostic>;

    /// Each source prepared, in the order given: a host may prepare them in parallel, and must
    /// give the same bytes for the same source whatever the machine.
    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Self::Refusal>>;
}

impl<I: Images + ?Sized> Images for &I {
    type Refusal = I::Refusal;

    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Self::Refusal>> {
        I::prepare(self, sources)
    }
}

/// One raster image to prepare.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Source<'a> {
    /// The file, for what is said about it.
    pub file: &'a ContentPath,
    /// Its kind, from its extension.
    pub media: MediaType,
    /// The hash of its bytes, which with the host's settings keys any cache of the work.
    pub blob: Hash,
    /// Its bytes.
    pub bytes: &'a [u8],
}

impl<'a> Source<'a> {
    /// The kinds the engine asks the host to prepare: every raster image a page may show.
    /// SVG is served as it is, sandboxed; an icon is not shown in a page.
    pub const RASTER: &'static [MediaType] =
        &[MediaType::Jpeg, MediaType::Png, MediaType::Webp, MediaType::Gif, MediaType::Avif];

    /// `bytes` of `file`, a `media` image whose hash is `blob`.
    pub const fn new(file: &'a ContentPath, media: MediaType, blob: Hash, bytes: &'a [u8]) -> Self {
        Self { file, media, blob, bytes }
    }
}

/// What the host made of one image.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Prepared {
    /// Served as it is: the host makes no copies of it, and says why.
    Kept {
        /// Its size.
        dimensions: Dimensions,
        /// Why it is kept.
        reason: Kept,
    },
    /// Copies of it, with its metadata gone.
    Varied(Variants),
}

/// Why an image is served as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kept {
    /// It is animated; a still copy would lose what it shows.
    Animated,
    /// The host does not decode its format (AVIF without a C decoder).
    Format,
}

/// The copies of one image.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Variants {
    /// Its size as shown: after its orientation.
    pub dimensions: Dimensions,
    /// Every copy: AVIF and a fallback every browser shows, at each width of the host's ladder
    /// that is no wider than the image.
    pub copies: Vec<Encoded>,
    /// A tiny blurred image of it to show while a copy loads, or `None` (an image with
    /// transparency would show it through).
    pub placeholder: Option<Encoded>,
}

impl Variants {
    /// An image of `dimensions` with these copies and this placeholder.
    pub const fn new(dimensions: Dimensions, copies: Vec<Encoded>, placeholder: Option<Encoded>) -> Self {
        Self { dimensions, copies, placeholder }
    }
}

/// Encoded bytes of one copy.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Encoded {
    /// Their format.
    pub media: MediaType,
    /// Their size.
    pub dimensions: Dimensions,
    /// The bytes.
    pub bytes: Vec<u8>,
}

impl Encoded {
    /// `bytes` of a `media` image of `dimensions`.
    pub const fn new(media: MediaType, dimensions: Dimensions, bytes: Vec<u8>) -> Self {
        Self { media, dimensions, bytes }
    }
}
