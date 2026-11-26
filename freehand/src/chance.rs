//! Seeded randomness: a drawing's every random draw comes from its seed.

use core::fmt::{self, Write as _};

/// A drawing's seed: the same seed draws the same marks.
///
/// A seed is named by text, hashed with FNV-1a, which reads its text one unit at a time: a seed
/// can be continued with more text, and the result is the seed of the whole text. A mark of
/// several lines names each by its own name and its index.
///
/// ```
/// use freehand::Seed;
///
/// assert_eq!(Seed::of("logo").and("f").and_number(12), Seed::of("logof12"));
/// ```
///
/// Writing into a seed with [`fmt::Write`] continues it the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed(pub u32);

impl Seed {
    /// The seed of no text: FNV-1a's offset basis.
    const BASIS: Self = Self(2_166_136_261);

    /// The seed named by `text`: FNV-1a over its UTF-16 code units, as the browser hashes it.
    pub fn of(text: &str) -> Self {
        Self::BASIS.and(text)
    }

    /// This seed continued with `text`: the seed of its own text followed by `text`.
    #[must_use]
    pub fn and(self, text: &str) -> Self {
        Self(text.encode_utf16().fold(self.0, |hash, unit| (hash ^ u32::from(unit)).wrapping_mul(16_777_619)))
    }

    /// This seed continued with `n` written in decimal, as `{name}{n}` would spell it.
    #[must_use]
    pub fn and_number(mut self, n: usize) -> Self {
        let _infallible = write!(self, "{n}");
        self
    }
}

impl fmt::Write for Seed {
    /// Continues the seed with `text`; never fails.
    fn write_str(&mut self, text: &str) -> fmt::Result {
        *self = self.and(text);
        Ok(())
    }
}

/// Where a hand's random draws come from.
pub trait Chance {
    /// The next draw, in `[0, 1)`.
    fn draw(&mut self) -> f64;
}

/// mulberry32: small, fast, and the same on every target.
#[derive(Clone, Debug)]
pub struct Mulberry(u32);

impl Mulberry {
    /// Draws seeded by `seed`.
    pub const fn new(seed: Seed) -> Self {
        Self(seed.0)
    }
}

impl Chance for Mulberry {
    fn draw(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let s = self.0;
        let mut t = (s ^ (s >> 15)).wrapping_mul(s | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 0x3d));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// The same draw every time: a steady hand, for geometry that is placed rather than drawn.
#[derive(Clone, Copy, Debug)]
pub struct Constant(pub f64);

impl Chance for Constant {
    fn draw(&mut self) -> f64 {
        self.0
    }
}
