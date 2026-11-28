//! Seeded randomness: a drawing's every random draw comes from its seed.

use core::fmt::{self, Write as _};

/// A drawing's seed: the same seed draws the same marks.
///
/// A seed is named by text, hashed with FNV-1a, which reads its text one unit at a time: a seed
/// can be continued with more text, and the result is the seed of the whole text. Anything that
/// displays names a seed, so a mark of several lines names each by its own name and its index.
///
/// ```
/// use freehand::Seed;
///
/// assert_eq!(Seed::of("logo").and("f").and(12), Seed::of("logof12"));
/// ```
///
/// Writing into a seed with [`fmt::Write`] continues it the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed(pub u32);

impl Seed {
    /// The seed of no text: FNV-1a's offset basis.
    const BASIS: Self = Self(2_166_136_261);

    /// The seed named by `name` as it displays: FNV-1a over its UTF-16 code units, as the browser
    /// hashes it.
    pub fn of(name: impl fmt::Display) -> Self {
        Self::BASIS.and(name)
    }

    /// This seed continued with `part` as it displays: the seed of its own text followed by
    /// `part`'s, so `and(12)` continues it as `{name}12` would spell it.
    #[must_use]
    pub fn and(mut self, part: impl fmt::Display) -> Self {
        let _infallible = write!(self, "{part}");
        self
    }
}

impl fmt::Write for Seed {
    /// Continues the seed with `text`; never fails.
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 = text.encode_utf16().fold(self.0, |hash, unit| (hash ^ u32::from(unit)).wrapping_mul(16_777_619));
        Ok(())
    }
}

/// Where a hand's random draws come from.
pub trait Chance {
    /// The next draw, in `[0, 1)`.
    fn draw(&mut self) -> f64;

    /// The draws of part `part` of a mark (a frame's second line is part `1`): the same for the
    /// same part however many draws this one has given, and independent of this one's.
    #[must_use]
    fn fork(&self, part: impl fmt::Display) -> Self
    where
        Self: Sized;
}

/// mulberry32: small, fast, and the same on every target.
#[derive(Clone, Debug)]
pub struct Mulberry {
    /// What it was seeded with, which its forks continue.
    seed: Seed,
    state: u32,
}

impl From<Seed> for Mulberry {
    /// Draws seeded by `seed`.
    fn from(seed: Seed) -> Self {
        Self { seed, state: seed.0 }
    }
}

impl Chance for Mulberry {
    fn draw(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        let s = self.state;
        let mut t = (s ^ (s >> 15)).wrapping_mul(s | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 0x3d));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    /// Draws seeded by this one's seed continued with `part`: part `1` of `frame` draws from
    /// `Seed::of("frame1")`.
    fn fork(&self, part: impl fmt::Display) -> Self {
        Self::from(self.seed.and(part))
    }
}

/// The same draw every time: a steady hand, for geometry that is placed rather than drawn.
#[derive(Clone, Copy, Debug)]
pub struct Constant(pub f64);

impl Chance for Constant {
    fn draw(&mut self) -> f64 {
        self.0
    }

    /// The same steady hand.
    fn fork(&self, _part: impl fmt::Display) -> Self {
        *self
    }
}
