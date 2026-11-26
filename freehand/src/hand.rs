//! A hand tracing a centreline: the wobble of matplotlib's xkcd mode, rough.js's bow, and a
//! mouse's tremor, with the pressure of a stroke that starts and ends light.

use alloc::vec::Vec;
use core::f64::consts::{PI, TAU};

use crate::centreline::Centreline;
use crate::chance::Chance;
use crate::point::{Point, Pressured};

/// How unsteady the hand is. `1` is the pencil of the page; the logo is drawn steadier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hand {
    /// Slow sideways drift and bow, as a multiple of the usual.
    pub wobble: f64,
    /// Fast small shake, in pixels.
    pub tremor: f64,
}

impl Default for Hand {
    fn default() -> Self {
        Self { wobble: 1.0, tremor: 0.9 }
    }
}

/// The waves one traced line is drawn with, drawn from chance once per line.
struct Waves {
    slow: (f64, f64),
    second: (f64, f64),
    shake: (f64, f64),
    amplitude: f64,
    bow: f64,
}

impl Hand {
    /// The line as this hand draws it: each point pushed sideways, with its pressure.
    pub fn trace(&self, line: &Centreline, chance: &mut impl Chance) -> Vec<Pressured> {
        let points = line.points();
        // each segment's length once: summed in order it is `line.length()`, bit for bit
        let segments: Vec<f64> =
            points.windows(2).map(|pair| if let [a, b] = pair { a.distance(*b) } else { 0.0 }).collect();
        let total = match segments.iter().sum::<f64>() {
            0.0 => 1.0,
            total => total,
        };
        let waves = self.waves(total, chance);
        let mut run = 0.0;
        let mut traced = Vec::with_capacity(points.len());
        for (i, point) in points.iter().enumerate() {
            if let Some(segment) = i.checked_sub(1).and_then(|b| segments.get(b)) {
                run += segment;
            }
            let t = run / total;
            let normal = normal(points, i);
            let shake =
                self.tremor * (0.7 * libm::sin(TAU * waves.shake.0 * t + waves.shake.1) + 0.5 * (chance.draw() - 0.5));
            let off = waves.amplitude
                * (0.8 * libm::sin(TAU * waves.slow.0 * t + waves.slow.1)
                    + 0.2 * libm::sin(TAU * waves.second.0 * t + waves.second.1))
                + waves.bow * libm::sin(PI * t)
                + shake;
            let pressure = 0.42 + 0.58 * libm::sin(PI * (0.06 + 0.88 * t));
            traced.push(Pressured::new(Point::new(point.x + normal.x * off, point.y + normal.y * off), pressure));
        }
        traced
    }

    fn waves(&self, total: f64, chance: &mut impl Chance) -> Waves {
        let slow = 0.5 + chance.draw() * 0.8;
        let second = 2.0 + chance.draw() * 1.5;
        let slow_phase = chance.draw() * TAU;
        let second_phase = chance.draw() * TAU;
        let shake = total / (10.0 + chance.draw() * 8.0);
        let shake_phase = chance.draw() * TAU;
        let amplitude = libm::fmin(1.8, 0.4 + total * 0.004) * self.wobble;
        let bow = (chance.draw() - 0.5) * libm::fmin(4.0, total * 0.01) * self.wobble;
        Waves { slow: (slow, slow_phase), second: (second, second_phase), shake: (shake, shake_phase), amplitude, bow }
    }
}

/// The unit normal at point `i`, from its neighbours.
fn normal(points: &[Point], i: usize) -> Point {
    let last = points.len().saturating_sub(1);
    let (Some(a), Some(b)) = (points.get(i.saturating_sub(1)), points.get((i + 1).min(last))) else {
        return Point::default();
    };
    let d = match a.distance(*b) {
        0.0 => 1.0,
        d => d,
    };
    Point::new(-(b.y - a.y) / d, (b.x - a.x) / d)
}
