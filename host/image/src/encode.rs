//! Pixels resized and encoded. Every step gives the same bytes on every machine: the resizer's
//! filter (Catmull-Rom) has no trigonometry and its SIMD paths are integer arithmetic giving the
//! scalar path's bytes; rav1e runs without assembly, on one thread and so one tile (ravif would
//! otherwise cut as many tiles as the machine has cores, and the bytes would follow the core
//! count); jpeg-encoder runs without its SIMD feature; PNG has no clock in it.

use engine::image::Encoded;
use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use model::{Dimensions, MediaType};

use crate::decode::{Alpha, Pixels};
use crate::error::Reason;
use crate::settings::Settings;
use crate::thumbhash::{self, Rgba, Thumbhash};

impl Pixels {
    /// Its size.
    pub(crate) const fn dimensions(&self) -> Dimensions {
        Dimensions::new(self.width, self.height)
    }

    /// The same image `width` pixels wide, its height in proportion.
    pub(crate) fn resize(&self, width: u32) -> Result<Self, Reason> {
        let height = u64::from(self.height) * u64::from(width) / u64::from(self.width.max(1));
        let height = u32::try_from(height).unwrap_or(u32::MAX).max(1);
        self.fit(width.max(1), height)
    }

    /// The same image at `width` by `height`, alpha premultiplied while it is filtered.
    fn fit(&self, width: u32, height: u32) -> Result<Self, Reason> {
        if (width, height) == (self.width, self.height) {
            return Ok(Self { width, height, alpha: self.alpha, bytes: self.bytes.clone() });
        }
        let kind = match self.alpha {
            Alpha::Opaque => PixelType::U8x3,
            Alpha::Transparent => PixelType::U8x4,
        };
        let source = ImageRef::new(self.width, self.height, &self.bytes, kind).map_err(Reason::Buffer)?;
        let mut target = Image::new(width, height, kind);
        let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::CatmullRom));
        Resizer::new().resize(&source, &mut target, &options).map_err(Reason::Resize)?;
        Ok(Self { width, height, alpha: self.alpha, bytes: target.into_vec() })
    }

    /// AVIF, with its alpha when it has some.
    pub(crate) fn avif(&self, settings: &Settings) -> Result<Encoded, Reason> {
        let encoder = ravif::Encoder::new()
            .with_quality(f32::from(settings.avif_quality))
            .with_alpha_quality(f32::from(settings.avif_quality))
            .with_speed(settings.avif_speed.clamp(1, 10))
            .with_num_threads(Some(1));
        let (width, height) = (self.width as usize, self.height as usize);
        let result = match self.alpha {
            Alpha::Opaque => {
                let pixels: Vec<ravif::RGB8> = self.bytes.as_chunks::<3>().0.iter().copied().map(Into::into).collect();
                encoder.encode_rgb(ravif::Img::new(&pixels, width, height))
            }
            Alpha::Transparent => {
                let pixels: Vec<ravif::RGBA8> = self.bytes.as_chunks::<4>().0.iter().copied().map(Into::into).collect();
                encoder.encode_rgba(ravif::Img::new(&pixels, width, height))
            }
        };
        let bytes = result.map_err(Reason::Avif)?.avif_file;
        Ok(Encoded::new(MediaType::Avif, self.dimensions(), bytes))
    }

    /// The copy every browser shows: JPEG, progressive, or PNG when the image has transparency.
    pub(crate) fn fallback(&self, settings: &Settings) -> Result<Encoded, Reason> {
        match self.alpha {
            Alpha::Opaque => self.jpeg(settings),
            Alpha::Transparent => self.png(),
        }
    }

    fn jpeg(&self, settings: &Settings) -> Result<Encoded, Reason> {
        let large = || Reason::Large { width: self.width, height: self.height };
        let width = u16::try_from(self.width).map_err(|_| large())?;
        let height = u16::try_from(self.height).map_err(|_| large())?;
        let mut bytes = Vec::new();
        let mut encoder = jpeg_encoder::Encoder::new(&mut bytes, settings.jpeg_quality.clamp(1, 100));
        encoder.set_progressive(true);
        encoder.encode(&self.bytes, width, height, jpeg_encoder::ColorType::Rgb).map_err(Reason::Jpeg)?;
        Ok(Encoded::new(MediaType::Jpeg, self.dimensions(), bytes))
    }

    fn png(&self) -> Result<Encoded, Reason> {
        let mut bytes = Vec::new();
        let colour = match self.alpha {
            Alpha::Opaque => ExtendedColorType::Rgb8,
            Alpha::Transparent => ExtendedColorType::Rgba8,
        };
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, PngFilter::Adaptive)
            .write_image(&self.bytes, self.width, self.height, colour)
            .map_err(Reason::Png)?;
        Ok(Encoded::new(MediaType::Png, self.dimensions(), bytes))
    }

    /// The image's thumbhash, drawn as a small opaque PNG; `None` when it has transparency.
    pub(crate) fn placeholder(&self) -> Result<Option<Encoded>, Reason> {
        if self.alpha == Alpha::Transparent {
            return Ok(None);
        }
        let longer = self.width.max(self.height).max(1);
        let scale = |side: u32| (u64::from(side) * u64::from(thumbhash::SIDE) / u64::from(longer)).max(1);
        let (width, height) = if longer > thumbhash::SIDE {
            (u32::try_from(scale(self.width)).unwrap_or(1), u32::try_from(scale(self.height)).unwrap_or(1))
        } else {
            (self.width, self.height)
        };
        let small = self.fit(width, height)?;
        let rgba: Vec<u8> = small.bytes.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, u8::MAX]).collect();
        let hash = Thumbhash::of(&Rgba { width: width as usize, height: height as usize, pixels: &rgba });
        let Some((width, height, drawn)) = hash.draw() else { return Ok(None) };
        let rgb: Vec<u8> = drawn.as_chunks::<4>().0.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect();
        let (width, height) = (u32::try_from(width).unwrap_or(1), u32::try_from(height).unwrap_or(1));
        let drawn = Self { width, height, alpha: Alpha::Opaque, bytes: rgb };
        drawn.png().map(Some)
    }
}
