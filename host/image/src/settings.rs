//! What the image work makes and refuses: the width ladder and the encoders' settings (what a
//! site owner may choose), and the [`Limits`] of the machine doing the work (what an operator
//! sets). Together with an image's hash they are the key of its work, so the same image under
//! the same settings is prepared once.

use std::fmt;
use std::num::NonZeroUsize;

use model::Hash;

use crate::error::Reason;

/// The settings of the image work. Every field changes the bytes made, so every field is in
/// [`Key`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Settings {
    /// The widths copies are made at, narrowest first. A width the image is not wider than is
    /// left out (no copy is wider than its image); the image's own width is added when the
    /// widest step left is more than a tenth narrower than it.
    pub widths: Vec<u32>,
    /// AVIF quality, 1 to 100.
    pub avif_quality: u8,
    /// rav1e's speed, 1 (slowest, smallest) to 10.
    pub avif_speed: u8,
    /// JPEG quality, 1 to 100.
    pub jpeg_quality: u8,
}

impl Default for Settings {
    /// Copies at 480, 960 and 1440 pixels (a 40rem column at one, two and three device pixels
    /// per CSS pixel, near enough), AVIF at quality 70 and speed 9, and JPEG at 80.
    fn default() -> Self {
        Self { widths: vec![480, 960, 1440], avif_quality: 70, avif_speed: 9, jpeg_quality: 80 }
    }
}

/// What one machine spends on the image work and the most it reads. Set by the operator of the
/// build, never by the site being built: a source is untrusted, so `ink.toml` may lower a limit
/// and never raise it. [`Key`] holds `pixels`, since a lowered budget must refuse an image an
/// earlier build kept; `workers` and `opening` change no byte and are not in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The most pixels an image may hold before it is refused, unread: `image.budget.pixels`.
    pub pixels: u64,
    /// The most threads the work runs on, so a machine with many cores does not hold as many
    /// copies in memory.
    pub workers: NonZeroUsize,
    /// The most images decoded at once, which bounds the full-size pixels held in memory.
    pub opening: NonZeroUsize,
}

impl Limits {
    /// Whether an image of `width` by `height` is within the pixel budget.
    pub(crate) fn admit(&self, width: u32, height: u32) -> Result<(), Reason> {
        let limit = self.pixels;
        if u64::from(width) * u64::from(height) > limit {
            return Err(Reason::Budget { width, height, limit });
        }
        Ok(())
    }
}

impl Default for Limits {
    /// 50 million pixels (a 48 megapixel phone photo is read, a decompression bomb is not),
    /// eight threads, and two images decoded at once.
    fn default() -> Self {
        Self {
            pixels: 50_000_000,
            workers: NonZeroUsize::MIN.saturating_add(7),
            opening: NonZeroUsize::MIN.saturating_add(1),
        }
    }
}

impl Settings {
    /// The widths of the copies of an image `width` pixels wide.
    pub fn ladder(&self, width: u32) -> Vec<u32> {
        let mut ladder: Vec<u32> =
            self.widths.iter().copied().filter(|&step| u64::from(step) * 10 < u64::from(width) * 9).collect();
        let widest = self.widths.iter().copied().max().unwrap_or(width).min(width);
        if ladder.last() != Some(&widest) {
            ladder.push(widest);
        }
        ladder
    }
}

/// The key of one image's work: the hash of its bytes, the settings, the pixel limit, and the
/// version of the work itself, hashed. The same key always gives the same bytes. A setting that
/// changes a byte of output is a field of [`Settings`] and is hashed below; a new one is added
/// to `of` in the same change, which a test checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(Hash);

impl Key {
    /// What changes with the codecs (their versions, read from `Cargo.lock` by the build script)
    /// or the way the work is done, so an old key never names new bytes.
    const VERSION: &'static [u8] = concat!("uses.ink/image/v1 ", env!("INK_IMAGE_CODECS"), "\0").as_bytes();

    /// The key of the image whose bytes hash to `blob`, under `settings` and `limits`.
    pub fn of(blob: Hash, settings: &Settings, limits: &Limits) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(Self::VERSION);
        hasher.update(blob.to_string().as_bytes());
        for width in &settings.widths {
            hasher.update(&width.to_le_bytes());
        }
        hasher.update(&[0, settings.avif_quality, settings.avif_speed, settings.jpeg_quality]);
        hasher.update(&limits.pixels.to_le_bytes());
        Self(hasher.finalize().into())
    }
}

impl From<Key> for engine::cache::Key {
    /// The key a cache keeps the work under.
    fn from(Key(digest): Key) -> Self {
        Self::new(engine::cache::Kind::Image, digest)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, out)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn key(settings: &Settings, limits: &Limits) -> Key {
        Key::of(Hash::of(b"an image"), settings, limits)
    }

    #[test]
    fn every_setting_that_changes_the_bytes_is_in_the_key() {
        let base = Settings::default();
        // destructured whole: a new field breaks this build until a variant below names it
        let Settings { widths, avif_quality, avif_speed, jpeg_quality } = base.clone();
        let changed = [
            Settings { widths: [widths.clone(), vec![2000]].concat(), ..base.clone() },
            Settings { widths: widths.iter().map(|width| width + 1).collect(), ..base.clone() },
            Settings { avif_quality: avif_quality - 1, ..base.clone() },
            Settings { avif_speed: avif_speed - 1, ..base.clone() },
            Settings { jpeg_quality: jpeg_quality - 1, ..base.clone() },
        ];
        let limits = Limits::default();
        let before = key(&base, &limits);
        for settings in &changed {
            assert_ne!(key(settings, &limits), before, "{settings:?}");
        }
        assert_eq!(key(&base.clone(), &limits), before);
    }

    #[test]
    fn the_pixel_limit_is_in_the_key_and_the_thread_limits_are_not() {
        let settings = Settings::default();
        let limits = Limits::default();
        let before = key(&settings, &limits);
        assert_ne!(key(&settings, &Limits { pixels: 1, ..limits }), before);
        assert_eq!(
            key(&settings, &Limits { workers: NonZeroUsize::MIN, opening: NonZeroUsize::MIN, ..limits }),
            before
        );
    }
}
