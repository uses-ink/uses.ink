//! The clean geometry a hand traces: where the line should go, before it wobbles.

use alloc::vec::Vec;
use core::f64::consts::PI;
use core::ops::Range;

use crate::chance::{Chance, Constant};
use crate::number::round;
use crate::point::Point;

/// Points along the middle of a line, about 5 px apart.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Centreline(Vec<Point>);

/// An ellipse as a hand goes round it: `count` points around `centre`, starting at angle `from`
/// (radians) and going `turns` times round, so a turn past one overlaps its start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    /// Its middle.
    pub centre: Point,
    /// Its half-width and half-height.
    pub radius: Point,
    /// The angle of its first point, in radians.
    pub from: f64,
    /// How many times round it goes.
    pub turns: f64,
    /// How many points it is drawn with.
    pub count: u32,
}

/// How far apart a straight line's points are.
const STEP: f64 = 5.0;

/// The most points a line is drawn with. A longer line (or a non-finite one) draws nothing, so a
/// mistaken size cannot exhaust memory: 100,000 points is a line 500,000 px long.
pub const MOST: u32 = 100_000;

impl Centreline {
    /// The line through `points` as given.
    pub fn through(points: impl IntoIterator<Item = Point>) -> Self {
        Self(points.into_iter().collect())
    }

    /// A straight line drawn by a quick hand: it starts a little before `from` and overshoots
    /// `to`, by amounts drawn from `chance`.
    pub fn straight(from: Point, to: Point, chance: &mut impl Chance) -> Self {
        let length = match from.distance(to) {
            0.0 => 1.0,
            length => length,
        };
        let ux = (to.x - from.x) / length;
        let uy = (to.y - from.y) / length;
        let start = Point::new(from.x - ux * chance.draw() * 2.0, from.y - uy * chance.draw() * 2.0);
        let end = Point::new(to.x + ux * (2.0 + chance.draw() * 5.0), to.y + uy * (2.0 + chance.draw() * 5.0));
        let n = libm::fmax(2.0, round(length / STEP));
        if !n.is_finite() || n > f64::from(MOST) {
            return Self::default();
        }
        Self(
            (0..=steps(n))
                .map(|i| {
                    let i = f64::from(i);
                    Point::new(start.x + ((end.x - start.x) * i) / n, start.y + ((end.y - start.y) * i) / n)
                })
                .collect(),
        )
    }

    /// A placed segment: a steady hand's straight line that ends exactly at `to`, for geometry
    /// whose corners must meet.
    pub fn segment(from: Point, to: Point) -> Self {
        let mut line = Self::straight(from, to, &mut Constant(0.5));
        line.0.pop();
        line.0.push(to);
        line
    }

    /// Segments from corner to corner, joined without repeating a corner.
    pub fn chain(corners: &[Point]) -> Self {
        let mut line = Self::default();
        for pair in corners.windows(2) {
            if let [from, to] = pair {
                let segment = Self::segment(*from, *to);
                let skip = usize::from(!line.0.is_empty());
                line.0.extend(segment.0.into_iter().skip(skip));
            }
        }
        line
    }

    /// A quadratic curve from `from` to `to`, pulled toward `control`, in `steps` steps.
    /// No steps, or more than [`MOST`], draw nothing.
    pub fn quad(from: Point, control: Point, to: Point, steps: u32) -> Self {
        if steps == 0 || steps > MOST {
            return Self::default();
        }
        let n = f64::from(steps);
        Self(
            (0..=steps)
                .map(|i| {
                    let t = f64::from(i) / n;
                    let u = 1.0 - t;
                    Point::new(
                        u * u * from.x + 2.0 * u * t * control.x + t * t * to.x,
                        u * u * from.y + 2.0 * u * t * control.y + t * t * to.y,
                    )
                })
                .collect(),
        )
    }

    /// The points around `ellipse`. More than [`MOST`] points draw nothing.
    pub fn ellipse(ellipse: Ellipse) -> Self {
        let Ellipse { centre, radius, from, turns, count } = ellipse;
        if count > MOST {
            return Self::default();
        }
        let last = f64::from(count.saturating_sub(1).max(1));
        Self(
            (0..count)
                .map(|i| {
                    let t = from + (f64::from(i) / last) * PI * 2.0 * turns;
                    Point::new(centre.x + libm::cos(t) * radius.x, centre.y + libm::sin(t) * radius.y)
                })
                .collect(),
        )
    }

    /// The points in `range`, or `None` when the line has fewer.
    pub fn part(&self, range: Range<usize>) -> Option<Self> {
        self.0.get(range).map(|points| Self(points.to_vec()))
    }

    /// The same line, scaled by `scale` and then moved by `offset`.
    #[must_use]
    pub fn placed(&self, offset: Point, scale: f64) -> Self {
        Self(self.0.iter().map(|p| Point::new(offset.x + p.x * scale, offset.y + p.y * scale)).collect())
    }

    /// The same line, moved by `offset`.
    #[must_use]
    pub fn moved(&self, offset: Point) -> Self {
        Self(self.0.iter().map(|p| Point::new(p.x + offset.x, p.y + offset.y)).collect())
    }

    /// Its points.
    pub fn points(&self) -> &[Point] {
        &self.0
    }

    /// How long it is, along its points.
    pub fn length(&self) -> f64 {
        self.0.windows(2).map(|pair| if let [a, b] = pair { a.distance(*b) } else { 0.0 }).sum()
    }
}

/// A count already rounded, at least 2 and at most [`MOST`], as a loop bound.
fn steps(n: f64) -> u32 {
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a rounded count from 2 to MOST")]
    let steps = n as u32;
    steps
}
