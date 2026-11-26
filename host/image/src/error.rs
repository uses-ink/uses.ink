//! Why an image could not be prepared. Each refusal leaves the crate as a [`Diagnostic`] with a
//! catalog code; the third-party error is kept as the source until then.

use std::fmt;

use model::{ContentPath, Diagnostic, Problem, Subject};

/// An image refused, with the file it is about.
#[derive(Debug)]
pub struct Refused {
    /// The file.
    pub file: ContentPath,
    /// Why.
    pub reason: Reason,
}

/// Why an image was refused.
#[derive(Debug)]
#[non_exhaustive]
pub enum Reason {
    /// It holds more pixels than the budget, said by its header before any is decoded.
    Budget {
        /// Its width.
        width: u32,
        /// Its height.
        height: u32,
        /// The most pixels an image may hold.
        limit: u64,
    },
    /// Its bytes are not an image of its kind.
    Decode(image::ImageError),
    /// Its header gives no size.
    Header(imagesize::ImageError),
    /// The pixels did not fill the resizer's buffer.
    Buffer(fast_image_resize::ImageBufferError),
    /// A copy could not be resized.
    Resize(fast_image_resize::ResizeError),
    /// A copy could not be encoded as AVIF.
    Avif(ravif::Error),
    /// A copy could not be encoded as JPEG.
    Jpeg(jpeg_encoder::EncodingError),
    /// A copy or the placeholder could not be encoded as PNG.
    Png(image::ImageError),
    /// A copy would be wider or taller than a JPEG can be.
    Large {
        /// Its width.
        width: u32,
        /// Its height.
        height: u32,
    },
}

impl Refused {
    /// `file`, refused for `reason`.
    pub(crate) fn new(file: &ContentPath, reason: impl Into<Reason>) -> Self {
        Self { file: file.clone(), reason: reason.into() }
    }
}

impl From<image::ImageError> for Reason {
    fn from(error: image::ImageError) -> Self {
        Self::Decode(error)
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget { width, height, limit } => {
                write!(out, "it is {width} by {height} pixels; an image may hold {limit}")
            }
            Self::Decode(error) | Self::Png(error) => fmt::Display::fmt(error, out),
            Self::Header(error) => fmt::Display::fmt(error, out),
            Self::Buffer(error) => fmt::Display::fmt(error, out),
            Self::Resize(error) => fmt::Display::fmt(error, out),
            Self::Avif(error) => fmt::Display::fmt(error, out),
            Self::Jpeg(error) => fmt::Display::fmt(error, out),
            Self::Large { width, height } => write!(out, "a JPEG cannot be {width} by {height} pixels"),
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}: {}", self.file, self.reason)
    }
}

impl std::error::Error for Refused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.reason {
            Reason::Decode(error) | Reason::Png(error) => Some(error),
            Reason::Header(error) => Some(error),
            Reason::Buffer(error) => Some(error),
            Reason::Resize(error) => Some(error),
            Reason::Avif(error) => Some(error),
            Reason::Jpeg(error) => Some(error),
            Reason::Budget { .. } | Reason::Large { .. } => None,
        }
    }
}

impl From<Refused> for Diagnostic {
    /// `image.budget.pixels` and `image.decode.failed` are the author's to fix; a copy that
    /// could not be made is `image.encode.failed`, ours.
    fn from(refused: Refused) -> Self {
        let path = refused.file.clone();
        let problem = match &refused.reason {
            Reason::Budget { width, height, limit } => Problem::ImageBudgetPixels {
                path: path.clone(),
                width: u64::from(*width),
                height: u64::from(*height),
                limit: *limit,
            },
            Reason::Decode(_) | Reason::Header(_) => {
                Problem::ImageDecodeFailed { path: path.clone(), reason: refused.reason.to_string() }
            }
            _ => Problem::ImageEncodeFailed { path: path.clone(), reason: refused.reason.to_string() },
        };
        Self::new(problem).on(Subject::File(path))
    }
}
