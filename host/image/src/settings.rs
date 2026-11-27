//! What the image work makes and refuses: the width ladder, the encoders' settings, the budget.
//! Together with an image's hash they are the key of its work, so the same image under the same
//! settings is prepared once.

use std::fmt;

use model::Hash;

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
    /// The most pixels an image may hold before it is refused, unread.
    pub pixels: u64,
}

impl Default for Settings {
    /// Copies at 480, 960 and 1440 pixels (a 40rem column at one, two and three device pixels
    /// per CSS pixel, near enough), AVIF at quality 70 and speed 9, JPEG at 80, and 50 million
    /// pixels: a 48 megapixel phone photo is read, a decompression bomb is not.
    fn default() -> Self {
        Self { widths: vec![480, 960, 1440], avif_quality: 70, avif_speed: 9, jpeg_quality: 80, pixels: 50_000_000 }
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

/// The key of one image's work: the hash of its bytes, the settings, and the version of the
/// work itself, hashed. The same key always gives the same bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(Hash);

impl Key {
    /// What changes with the encoders or the way the work is done, so an old key never names
    /// new bytes.
    const VERSION: &'static [u8] =
        b"uses.ink/image/v1 image 0.25.10 fast_image_resize 6.2.0 ravif 0.13.0 jpeg-encoder 0.7.1\0";

    /// The key of the image whose bytes hash to `blob`, under `settings`.
    pub fn of(blob: Hash, settings: &Settings) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(Self::VERSION);
        hasher.update(blob.to_string().as_bytes());
        for width in &settings.widths {
            hasher.update(&width.to_le_bytes());
        }
        hasher.update(&[0, settings.avif_quality, settings.avif_speed, settings.jpeg_quality]);
        hasher.update(&settings.pixels.to_le_bytes());
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
