//! Seeded randomness: a drawing's every random draw comes from its seed.

/// A drawing's seed: the same seed draws the same marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed(pub u32);

impl Seed {
    /// The seed named by `text`: FNV-1a over its UTF-16 code units, as the browser hashes it.
    pub fn of(text: &str) -> Self {
        let mut hash: u32 = 2_166_136_261;
        for unit in text.encode_utf16() {
            hash = (hash ^ u32::from(unit)).wrapping_mul(16_777_619);
        }
        Self(hash)
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
