//! perfect-freehand's `getStroke`, ported: pressured points to the polygon around their ink.
//! Constants, loop bounds and the order of every operation are kept, so the outline is the one
//! the JavaScript draws.

use alloc::vec::Vec;
use core::f64::consts::PI;

use crate::point::{Point, Pressured};

/// PI nudged, as perfect-freehand nudges it against rendering artefacts.
const FIXED_PI: f64 = PI + 0.0001;
const START_CAP_SEGMENTS: f64 = 13.0;
const END_CAP_SEGMENTS: f64 = 29.0;
const CORNER_CAP_SEGMENTS: f64 = 13.0;
/// Pixels at the end of a stroke skipped as noise.
const END_NOISE: f64 = 3.0;
const MIN_STREAMLINE_T: f64 = 0.15;
const STREAMLINE_T_RANGE: f64 = 0.85;
const MIN_RADIUS: f64 = 0.01;
const FIRST_PRESSURE: f64 = 0.25;
const PRESSURE: f64 = 0.5;
const RATE_OF_PRESSURE_CHANGE: f64 = 0.275;
const UNIT: Point = Point::new(1.0, 1.0);

/// How one end of a stroke finishes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct End {
    /// Round (`true`) or flat, when the end is not tapered.
    pub cap: bool,
    /// How far the stroke thins toward this end, in pixels; `0` for none.
    pub taper: Taper,
    /// The easing of the taper.
    pub easing: Easing,
}

/// How a value from 0 to 1 is eased.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    /// As it is.
    Linear,
    /// Fast, then slow: `t (2 - t)`.
    OutQuad,
    /// Faster, then slower: `(t - 1)^3 + 1`.
    OutCubic,
}

impl Easing {
    /// `t`, eased.
    pub fn apply(self, t: f64) -> f64 {
        match self {
            Self::Linear => t,
            Self::OutQuad => t * (2.0 - t),
            Self::OutCubic => {
                let t = t - 1.0;
                t * t * t + 1.0
            }
        }
    }
}

/// A taper's length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Taper {
    /// No taper.
    None,
    /// The whole stroke, or its size if longer.
    Whole,
    /// This many pixels.
    Length(f64),
}

/// A cap, for the common case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    /// Rounded.
    Round,
    /// Squared off.
    Flat,
}

impl End {
    /// An untapered end with `cap`, easing as perfect-freehand's start does.
    pub fn first(cap: Cap) -> Self {
        Self { cap: cap == Cap::Round, taper: Taper::None, easing: Easing::OutQuad }
    }

    /// An untapered end with `cap`, easing as perfect-freehand's end does.
    pub fn last(cap: Cap) -> Self {
        Self { cap: cap == Cap::Round, taper: Taper::None, easing: Easing::OutCubic }
    }
}

/// perfect-freehand's options, with its defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    /// The diameter of the stroke.
    pub size: f64,
    /// How much pressure changes the size.
    pub thinning: f64,
    /// How much the edges are softened.
    pub smoothing: f64,
    /// How much the points are pulled toward the line behind them.
    pub streamline: f64,
    /// Pressure from speed instead of from the points.
    pub simulate_pressure: bool,
    /// The easing of pressure into size.
    pub easing: Easing,
    /// The start of the stroke.
    pub start: End,
    /// The end of the stroke.
    pub end: End,
    /// Whether the stroke is finished, so its last point is kept as given.
    pub complete: bool,
}

impl Default for Outline {
    fn default() -> Self {
        Self {
            size: 16.0,
            thinning: 0.5,
            smoothing: 0.5,
            streamline: 0.5,
            simulate_pressure: true,
            easing: Easing::Linear,
            start: End::first(Cap::Round),
            end: End::last(Cap::Round),
            complete: false,
        }
    }
}

/// A point of the stroke after streamlining.
#[derive(Clone, Copy, Debug)]
struct StrokePoint {
    point: Point,
    pressure: f64,
    vector: Point,
    distance: f64,
    running: f64,
}

impl Outline {
    /// The polygon around the ink of `points`.
    pub fn around(&self, points: &[Pressured]) -> Vec<Point> {
        self.outline(&self.stroke_points(points))
    }

    #[expect(clippy::too_many_lines, reason = "perfect-freehand's one loop, kept whole to read against the original")]
    fn stroke_points(&self, input: &[Pressured]) -> Vec<StrokePoint> {
        let Some(first) = input.first() else { return Vec::new() };
        let t = MIN_STREAMLINE_T + (1.0 - self.streamline) * STREAMLINE_T_RANGE;
        let mut pts: Vec<Pressured> = input.to_vec();
        // Two points become five, so a tapered short line is not a dash.
        if let [a, b] = input {
            pts = core::iter::once(*a)
                .chain((1..5).map(|i| Pressured::bare(a.at.lerp(b.at, f64::from(i) / 4.0))))
                .collect();
        }
        if pts.len() == 1 {
            pts.push(Pressured { at: first.at + UNIT, pressure: first.pressure });
        }
        let pressure = |p: &Pressured, default: f64| p.pressure.filter(|v| *v >= 0.0).unwrap_or(default);
        let mut out = Vec::with_capacity(pts.len());
        out.push(StrokePoint {
            point: first.at,
            pressure: pressure(first, FIRST_PRESSURE),
            vector: UNIT,
            distance: 0.0,
            running: 0.0,
        });
        let mut reached = false;
        let mut running = 0.0;
        let mut prev = out.first().map_or(first.at, |p| p.point);
        let max = pts.len() - 1;
        for (i, input) in pts.iter().enumerate().skip(1) {
            let point = if self.complete && i == max { input.at } else { prev.lerp(input.at, t) };
            if prev == point {
                continue;
            }
            let distance = point.distance(prev);
            running += distance;
            if i < max && !reached {
                if running < self.size {
                    continue;
                }
                reached = true;
            }
            out.push(StrokePoint {
                point,
                pressure: pressure(input, PRESSURE),
                vector: (prev - point).unit(),
                distance,
                running,
            });
            prev = point;
        }
        let second = out.get(1).map_or(Point::default(), |p| p.vector);
        if let Some(first) = out.first_mut() {
            first.vector = second;
        }
        out
    }

    fn taper(taper: Taper, size: f64, total: f64) -> f64 {
        match taper {
            Taper::None => 0.0,
            Taper::Whole => libm::fmax(size, total),
            Taper::Length(length) => length,
        }
    }

    fn radius(&self, pressure: f64) -> f64 {
        self.size * self.easing.apply(0.5 - self.thinning * (0.5 - pressure))
    }

    fn simulated(&self, previous: f64, distance: f64) -> f64 {
        let speed = libm::fmin(1.0, distance / self.size);
        let rate = libm::fmin(1.0, 1.0 - speed);
        libm::fmin(1.0, previous + (rate - previous) * (speed * RATE_OF_PRESSURE_CHANGE))
    }
}

/// The two sides of the outline as they are collected.
struct Sides {
    left: Vec<Point>,
    right: Vec<Point>,
    prev_left: Point,
    prev_right: Point,
    min_distance: f64,
}

impl Sides {
    /// A rounded cap at a sharp corner: both sides turn half a circle around `point`.
    fn corner(&mut self, point: Point, offset: Point) {
        let step = 1.0 / CORNER_CAP_SEGMENTS;
        let mut t = 0.0;
        while t <= 1.0 {
            self.prev_left = (point - offset).rotate(point, FIXED_PI * t);
            self.left.push(self.prev_left);
            self.prev_right = (point + offset).rotate(point, FIXED_PI * -t);
            self.right.push(self.prev_right);
            t += step;
        }
    }

    /// A regular point on each side, kept when it moved far enough from the last one kept.
    fn regular(&mut self, point: Point, offset: Point, early: bool) {
        let left = point - offset;
        if early || self.prev_left.distance_squared(left) > self.min_distance {
            self.left.push(left);
            self.prev_left = left;
        }
        let right = point + offset;
        if early || self.prev_right.distance_squared(right) > self.min_distance {
            self.right.push(right);
            self.prev_right = right;
        }
    }
}

impl Outline {
    #[expect(clippy::too_many_lines, reason = "perfect-freehand's one loop, kept whole to read against the original")]
    fn outline(&self, points: &[StrokePoint]) -> Vec<Point> {
        let (Some(first), Some(last)) = (points.first(), points.last()) else { return Vec::new() };
        if self.size <= 0.0 {
            return Vec::new();
        }
        let total = last.running;
        let taper_start = Self::taper(self.start.taper, self.size, total);
        let taper_end = Self::taper(self.end.taper, self.size, total);
        let mut sides = Sides {
            left: Vec::new(),
            right: Vec::new(),
            prev_left: first.point,
            prev_right: first.point,
            // `Math.pow(x, 2)` is `x * x` in the browser's fdlibm.
            min_distance: (self.size * self.smoothing) * (self.size * self.smoothing),
        };
        #[expect(clippy::manual_midpoint, reason = "the browser's `(a + b) / 2`, for the same bits")]
        let mut prev_pressure = points.iter().take(10).fold(first.pressure, |acc, p| {
            let pressure = if self.simulate_pressure { self.simulated(acc, p.distance) } else { p.pressure };
            (acc + pressure) / 2.0
        });
        let mut radius = self.radius(last.pressure);
        let mut first_radius = None;
        let mut prev_vector = first.vector;
        let mut prev_sharp = false;
        for (i, p) in points.iter().enumerate() {
            let is_last = i == points.len() - 1;
            if !is_last && total - p.running < END_NOISE {
                continue;
            }
            let mut pressure = p.pressure;
            radius = if self.thinning == 0.0 {
                self.size / 2.0
            } else {
                if self.simulate_pressure {
                    pressure = self.simulated(prev_pressure, p.distance);
                }
                self.radius(pressure)
            };
            first_radius.get_or_insert(radius);
            let start = if p.running < taper_start { self.start.easing.apply(p.running / taper_start) } else { 1.0 };
            let end = if total - p.running < taper_end {
                self.end.easing.apply((total - p.running) / taper_end)
            } else {
                1.0
            };
            radius = libm::fmax(MIN_RADIUS, radius * libm::fmin(start, end));
            let next_vector = if is_last { p.vector } else { points.get(i + 1).map_or(p.vector, |n| n.vector) };
            let next_dot = if is_last { 1.0 } else { p.vector.dot(next_vector) };
            let sharp = p.vector.dot(prev_vector) < 0.0 && !prev_sharp;
            let next_sharp = next_dot < 0.0;
            if sharp || next_sharp {
                sides.corner(p.point, prev_vector.perpendicular() * radius);
                if next_sharp {
                    prev_sharp = true;
                }
                continue;
            }
            prev_sharp = false;
            if is_last {
                let offset = p.vector.perpendicular() * radius;
                sides.left.push(p.point - offset);
                sides.right.push(p.point + offset);
                continue;
            }
            let offset = next_vector.lerp(p.vector, next_dot).perpendicular() * radius;
            sides.regular(p.point, offset, i <= 1);
            prev_pressure = pressure;
            prev_vector = p.vector;
        }
        let ends = Ends { first: first.point, last: last.point, vector: last.vector, radius, taper_start, taper_end };
        if points.len() == 1 {
            if (taper_start == 0.0 && taper_end == 0.0) || self.complete {
                return dot(first.point, first_radius.unwrap_or(radius));
            }
            return finish(sides, Vec::new(), Vec::new());
        }
        let start_cap = self.start_cap(&ends, &sides);
        let end_cap = self.end_cap(&ends);
        finish(sides, end_cap, start_cap)
    }
}

/// What the caps are drawn from.
struct Ends {
    first: Point,
    last: Point,
    vector: Point,
    radius: f64,
    taper_start: f64,
    taper_end: f64,
}

impl Outline {
    fn start_cap(&self, ends: &Ends, sides: &Sides) -> Vec<Point> {
        if ends.taper_start != 0.0 {
            return Vec::new();
        }
        let (Some(left), Some(right)) = (sides.left.first(), sides.right.first()) else { return Vec::new() };
        if self.start.cap {
            return turns(START_CAP_SEGMENTS, true, |t| right.rotate(ends.first, FIXED_PI * t));
        }
        let corners = *left - *right;
        let (a, b) = (corners * 0.5, corners * 0.51);
        alloc::vec![ends.first - a, ends.first - b, ends.first + b, ends.first + a]
    }

    fn end_cap(&self, ends: &Ends) -> Vec<Point> {
        let direction = Point::new(-ends.vector.x, -ends.vector.y).perpendicular();
        if ends.taper_end != 0.0 {
            return alloc::vec![ends.last];
        }
        if self.end.cap {
            let start = ends.last + direction * ends.radius;
            return turns(END_CAP_SEGMENTS, false, |t| start.rotate(ends.last, FIXED_PI * 3.0 * t));
        }
        let (a, b) = (direction * ends.radius, direction * (ends.radius * 0.99));
        alloc::vec![ends.last + a, ends.last + b, ends.last - b, ends.last - a]
    }
}

/// `point(t)` for `t` from one step up to 1, stepping as perfect-freehand steps (by adding, so
/// the count is the float loop's); `through` includes a `t` that lands on 1.
fn turns(segments: f64, through: bool, point: impl Fn(f64) -> Point) -> Vec<Point> {
    let step = 1.0 / segments;
    let mut out = Vec::new();
    let mut t = step;
    while if through { t <= 1.0 } else { t < 1.0 } {
        out.push(point(t));
        t += step;
    }
    out
}

/// A dot, for a stroke of one point.
fn dot(centre: Point, radius: f64) -> Vec<Point> {
    let start = centre + (centre - (centre + UNIT)).perpendicular().unit() * -radius;
    turns(START_CAP_SEGMENTS, true, |t| start.rotate(centre, FIXED_PI * 2.0 * t))
}

/// Left side, end cap, right side backwards, start cap: the winding perfect-freehand returns.
fn finish(sides: Sides, end_cap: Vec<Point>, start_cap: Vec<Point>) -> Vec<Point> {
    let mut out = sides.left;
    out.extend(end_cap);
    out.extend(sides.right.into_iter().rev());
    out.extend(start_cap);
    out
}
