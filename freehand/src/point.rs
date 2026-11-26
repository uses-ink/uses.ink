//! Points and the vector arithmetic the outline needs, in the order perfect-freehand does it.

use core::ops::{Add, Mul, Sub};

use crate::chance::Chance;

/// A point, or a vector between two.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    /// Across, to the right.
    pub x: f64,
    /// Down.
    pub y: f64,
}

/// An upright rectangle between two corners: a box a mark is drawn around or behind.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rectangle {
    /// Its top left corner.
    pub top_left: Point,
    /// Its bottom right corner.
    pub bottom_right: Point,
}

impl Rectangle {
    /// Its four corners, clockwise on screen from the top left.
    pub const fn corners(self) -> [Point; 4] {
        let Self { top_left: a, bottom_right: b } = self;
        [Point::new(a.x, a.y), Point::new(b.x, a.y), Point::new(b.x, b.y), Point::new(a.x, b.y)]
    }

    /// Its corners as a quick hand places them: each misses its place by up to `slack / 2` on
    /// each axis, drawn from `chance` across then down, corner by corner.
    pub(crate) fn jittered(self, slack: f64, chance: &mut impl Chance) -> [Point; 4] {
        self.corners().map(|corner| {
            let dx = (chance.draw() - 0.5) * slack;
            let dy = (chance.draw() - 0.5) * slack;
            Point::new(corner.x + dx, corner.y + dy)
        })
    }
}

impl Point {
    /// The point at `x`, `y`.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// The distance to `other`.
    pub fn distance(self, other: Self) -> f64 {
        libm::sqrt(self.distance_squared(other))
    }

    pub(crate) fn distance_squared(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// Its length: `sqrt(x² + y²)`, exact IEEE arithmetic on every target, and much faster than
    /// `hypot`, which only matters for lengths past 1e150 that no drawing has.
    pub(crate) fn length(self) -> f64 {
        libm::sqrt(self.x * self.x + self.y * self.y)
    }

    /// The unit vector of this one; a zero vector gives NaN, as in perfect-freehand.
    pub(crate) fn unit(self) -> Self {
        let length = self.length();
        Self::new(self.x / length, self.y / length)
    }

    /// Turned a quarter, clockwise on screen.
    pub(crate) fn perpendicular(self) -> Self {
        Self::new(self.y, -self.x)
    }

    pub(crate) fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// The point `t` of the way to `other`.
    pub(crate) fn lerp(self, other: Self, t: f64) -> Self {
        self + (other - self) * t
    }

    /// Turned by `angle` radians around `centre`.
    pub(crate) fn rotate(self, centre: Self, angle: f64) -> Self {
        let (s, c) = (libm::sin(angle), libm::cos(angle));
        let px = self.x - centre.x;
        let py = self.y - centre.y;
        Self::new(px * c - py * s + centre.x, px * s + py * c + centre.y)
    }

    /// Halfway to `other`.
    #[expect(clippy::manual_midpoint, reason = "the browser's `(a + b) / 2`, for the same bits")]
    pub(crate) fn mid(self, other: Self) -> Self {
        Self::new((self.x + other.x) / 2.0, (self.y + other.y) / 2.0)
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f64> for Point {
    type Output = Self;
    fn mul(self, n: f64) -> Self {
        Self::new(self.x * n, self.y * n)
    }
}

/// A traced point with the pressure the hand put on it, from 0 to 1. A point with no pressure
/// of its own takes perfect-freehand's default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pressured {
    /// Where.
    pub at: Point,
    /// How hard, when known.
    pub pressure: Option<f64>,
}

impl Pressured {
    /// A point pressed `pressure` hard.
    pub const fn new(at: Point, pressure: f64) -> Self {
        Self { at, pressure: Some(pressure) }
    }

    /// A point with no pressure of its own.
    pub const fn bare(at: Point) -> Self {
        Self { at, pressure: None }
    }
}
