//! An image's bytes read into pixels: its size checked against the budget from its header
//! before a pixel is decoded, its EXIF orientation applied, every bit of metadata left behind
//! (only pixels go on), an alpha channel that is opaque everywhere dropped.

use std::io::Cursor;

use engine::image::{Kept, Source};
use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use model::{Dimensions, MediaType};

use crate::error::{Reason, Refused};
use crate::settings::Limits;

/// Pixels, row by row: RGB, or RGBA when some pixel is not opaque.
#[derive(Debug)]
pub(crate) struct Pixels {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) alpha: Alpha,
    pub(crate) bytes: Vec<u8>,
}

/// Whether an image has transparency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alpha {
    /// Every pixel is opaque: three bytes a pixel.
    Opaque,
    /// Some pixel is not: four bytes a pixel.
    Transparent,
}

/// What reading an image gave.
#[derive(Debug)]
pub(crate) enum Decoded {
    /// It is served as it is.
    Kept(Dimensions, Kept),
    /// Its pixels.
    Pixels(Pixels),
}

impl Decoded {
    /// `source` read within `limits`.
    pub(crate) fn read(source: &Source<'_>, limits: &Limits) -> Result<Self, Refused> {
        let refuse = |reason: Reason| Refused::new(source.file, reason);
        let format = match source.media {
            MediaType::Jpeg => ImageFormat::Jpeg,
            MediaType::Png => ImageFormat::Png,
            MediaType::Webp => ImageFormat::WebP,
            MediaType::Gif => ImageFormat::Gif,
            // no pure Rust AVIF decoder: its size from its header, its bytes as they are
            _ => {
                let size = imagesize::blob_size(source.bytes).map_err(|error| refuse(Reason::Header(error)))?;
                let (width, height) = (Self::side(size.width), Self::side(size.height));
                return Ok(Self::Kept(Dimensions::new(width, height), Kept::Format));
            }
        };
        let reader = || ImageReader::with_format(Cursor::new(source.bytes), format);
        let (width, height) = reader().into_dimensions().map_err(|error| refuse(error.into()))?;
        let limit = limits.pixels;
        if u64::from(width) * u64::from(height) > limit {
            return Err(refuse(Reason::Budget { width, height, limit }));
        }
        if Self::animated(source.bytes, format).map_err(|error| refuse(error.into()))? {
            return Ok(Self::Kept(Dimensions::new(width, height), Kept::Animated));
        }
        let mut decoder_limits = image::Limits::default();
        decoder_limits.max_image_width = Some(width);
        decoder_limits.max_image_height = Some(height);
        // the decoded image in 16-bit RGBA, and as much again for the decoder's own buffers
        decoder_limits.max_alloc = Some(limit.saturating_mul(16));
        let mut decoder = reader().into_decoder().map_err(|error| refuse(error.into()))?;
        decoder.set_limits(decoder_limits).map_err(|error| refuse(error.into()))?;
        let orientation = decoder.orientation().map_err(|error| refuse(error.into()))?;
        let mut image = DynamicImage::from_decoder(decoder).map_err(|error| refuse(error.into()))?;
        image.apply_orientation(orientation);
        Ok(Self::Pixels(Pixels::from(image)))
    }

    /// Whether the image has more than one frame.
    fn animated(bytes: &[u8], format: ImageFormat) -> image::ImageResult<bool> {
        Ok(match format {
            ImageFormat::Gif => GifDecoder::new(Cursor::new(bytes))?.into_frames().take(2).count() > 1,
            ImageFormat::WebP => WebPDecoder::new(Cursor::new(bytes))?.has_animation(),
            _ => false,
        })
    }

    /// A side from a header, which no image this large reaches.
    fn side(pixels: usize) -> u32 {
        u32::try_from(pixels).unwrap_or(u32::MAX)
    }
}

impl From<DynamicImage> for Pixels {
    /// Eight bits a channel; RGBA only when some pixel is not opaque.
    fn from(image: DynamicImage) -> Self {
        let (width, height) = (image.width(), image.height());
        if image.color().has_alpha() {
            let rgba = image.into_rgba8();
            if rgba.pixels().any(|pixel| pixel.0[3] != u8::MAX) {
                return Self { width, height, alpha: Alpha::Transparent, bytes: rgba.into_raw() };
            }
            let rgb = DynamicImage::ImageRgba8(rgba).into_rgb8();
            return Self { width, height, alpha: Alpha::Opaque, bytes: rgb.into_raw() };
        }
        Self { width, height, alpha: Alpha::Opaque, bytes: image.into_rgb8().into_raw() }
    }
}
