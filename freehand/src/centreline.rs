//! The clean geometry a hand traces: where the line should go, before it wobbles.

use alloc::vec::Vec;
use core::f64::consts::PI;
use core::ops::Range;

use crate::chance::{Chance, Constant};
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

impl FromIterator<Point> for Centreline {
    /// The line through the points as given.
    fn from_iter<I: IntoIterator<Item = Point>>(points: I) -> Self {
        Self(points.into_iter().collect())
    }
}

impl From<Ellipse> for Centreline {
    /// The points around the ellipse. More than [`MOST`] points draw nothing.
    fn from(ellipse: Ellipse) -> Self {
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
}

impl Centreline {
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
        let Some(count) = Count::of(length, STEP, 2.0) else { return Self::default() };
        let n = f64::from(count);
        Self(
            (0..=u32::from(count))
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
    pub fn chain(corners: impl AsRef<[Point]>) -> Self {
        let mut line = Self::default();
        for pair in corners.as_ref().windows(2) {
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

    /// The unit normal at point `i`, from its neighbours; zero past its end.
    pub(crate) fn normal(&self, i: usize) -> Point {
        let last = self.0.len().saturating_sub(1);
        let (Some(a), Some(b)) = (self.0.get(i.saturating_sub(1)), self.0.get((i + 1).min(last))) else {
            return Point::default();
        };
        let d = match a.distance(*b) {
            0.0 => 1.0,
            d => d,
        };
        Point::new(-(b.y - a.y) / d, (b.x - a.x) / d)
    }

    /// How long it is, along its points.
    pub fn length(&self) -> f64 {
        self.0.windows(2).map(|pair| if let [a, b] = pair { a.distance(*b) } else { 0.0 }).sum()
    }
}

/// How many steps a length is drawn in: `Math.round(length / spacing)`, at least `least` and at
/// most [`MOST`], so a whole number a loop can count to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Count(f64);

impl Count {
    /// The count for `length` at `spacing`; `None` when it is not finite or past [`MOST`], so an
    /// absurd line draws nothing rather than exhausting memory.
    pub(crate) fn of(length: f64, spacing: f64, least: f64) -> Option<Self> {
        let ratio = length / spacing;
        if !ratio.is_finite() {
            return None;
        }
        // `Math.round`: halves round up
        let floor = libm::floor(ratio);
        let rounded = if ratio - floor >= 0.5 { floor + 1.0 } else { floor };
        let n = libm::fmax(least, rounded);
        (n.is_finite() && n <= f64::from(MOST)).then_some(Self(n))
    }
}

impl From<Count> for f64 {
    fn from(count: Count) -> Self {
        count.0
    }
}

impl From<Count> for u32 {
    fn from(count: Count) -> Self {
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a whole count up to MOST")]
        let whole = count.0 as u32;
        whole
    }
}

#[cfg(test)]
mod count_test {
    use super::Count;

    #[test]
    fn a_length_that_is_not_a_number_has_no_count() {
        assert!(Count::of(f64::NAN, 2.0, 1.0).is_none());
        assert!(Count::of(1.0, 0.0, 1.0).is_none(), "an infinite ratio");
        assert_eq!(Count::of(10.0, 2.0, 1.0).map(f64::from), Some(5.0));
    }
}
