//! A JPEG read through zune-jpeg directly. `image` pins an older zune-jpeg that misreads the
//! optimized-table files this crate writes (decision 0040), so its own JPEG feature is off and
//! this reader hands the pixels to the same pipeline as any other format.

use image::error::{DecodingError, ImageError};
use image::metadata::Orientation;
use image::{DynamicImage, ImageBuffer, ImageFormat};
use zune_jpeg::JpegDecoder;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

/// A JPEG whose headers are read: its size and orientation are known, its pixels are not.
pub(crate) struct Jpeg<'a> {
    decoder: JpegDecoder<ZCursor<&'a [u8]>>,
    width: u32,
    height: u32,
    grey: bool,
}

impl<'a> Jpeg<'a> {
    /// `bytes`, with only the headers read. The size is the budget's to judge; the budget, not
    /// the decoder, bounds what is allocated.
    pub(crate) fn open(bytes: &'a [u8]) -> Result<Self, ImageError> {
        let options =
            DecoderOptions::default().set_strict_mode(false).set_max_width(usize::MAX).set_max_height(usize::MAX);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
        decoder.decode_headers().map_err(Self::refuse)?;
        let (width, height) =
            decoder.dimensions().ok_or_else(|| Self::refuse(DecodeErrors::FormatStatic("it has no size")))?;
        let grey = matches!(decoder.input_colorspace(), Some(ColorSpace::Luma | ColorSpace::LumaA));
        let out = if grey { ColorSpace::Luma } else { ColorSpace::RGB };
        let options = decoder.options().jpeg_set_out_colorspace(out);
        decoder.set_options(options);
        // a JPEG cannot be wider or taller than 65535, so these fit
        Ok(Self {
            decoder,
            width: u32::try_from(width).unwrap_or(u32::MAX),
            height: u32::try_from(height).unwrap_or(u32::MAX),
            grey,
        })
    }

    /// Its width and height.
    pub(crate) const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// How its EXIF orientation says to turn it; none when it says nothing.
    pub(crate) fn orientation(&self) -> Orientation {
        self.decoder.exif().and_then(|exif| Orientation::from_exif_chunk(exif)).unwrap_or(Orientation::NoTransforms)
    }

    /// Its pixels, grey or RGB; CMYK and YCCK are converted.
    pub(crate) fn decode(mut self) -> Result<DynamicImage, ImageError> {
        let bytes = self.decoder.decode().map_err(Self::refuse)?;
        let (width, height) = (self.width, self.height);
        let mismatch =
            || ImageError::Decoding(DecodingError::new(ImageFormat::Jpeg.into(), "its pixels do not fill its size"));
        if self.grey {
            ImageBuffer::from_raw(width, height, bytes).map(DynamicImage::ImageLuma8).ok_or_else(mismatch)
        } else {
            ImageBuffer::from_raw(width, height, bytes).map(DynamicImage::ImageRgb8).ok_or_else(mismatch)
        }
    }

    /// The decoder's error as the pipeline's.
    fn refuse(error: DecodeErrors) -> ImageError {
        ImageError::Decoding(DecodingError::new(ImageFormat::Jpeg.into(), error))
    }
}
