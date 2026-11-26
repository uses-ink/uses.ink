//! A highlighter pass: an outline jittered, then smoothed, so a fill reads as one marker stroke
//! that misses its lines.

use alloc::vec::Vec;

use crate::Tool;
use crate::centreline::{Centreline, Count};
use crate::chance::Chance;
use crate::hand::Hand;
use crate::path::Path;
use crate::point::Point;

/// A highlighter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlighter {
    /// How far apart its points are along an edge, in pixels.
    pub spacing: f64,
    /// How far each point strays, in pixels, across both directions.
    pub jitter: f64,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self { spacing: 14.0, jitter: 1.6 }
    }
}

impl Highlighter {
    /// One pass over the shape with these corners. An edge needing more than [`MOST`](crate::MOST) points,
    /// or a non-finite corner, draws nothing.
    pub fn fill(&self, corners: &[Point], chance: &mut impl Chance) -> Path {
        let mut points = Vec::new();
        for (i, from) in corners.iter().enumerate() {
            let Some(to) = corners.get((i + 1) % corners.len()) else { continue };
            // a non-finite or absurd edge draws nothing, as an over-long line does
            let Some(count) = Count::of(from.distance(*to), self.spacing, 1.0) else { return Path::default() };
            let count = f64::from(count);
            let mut step = 0.0;
            while step < count {
                let t = step / count;
                let x = from.x + (to.x - from.x) * t + (chance.draw() - 0.5) * self.jitter;
                let y = from.y + (to.y - from.y) * t + (chance.draw() - 0.5) * self.jitter;
                points.push(Point::new(x, y));
                step += 1.0;
            }
        }
        Path::around(&points)
    }
}

impl Tool for Highlighter {
    /// One pass around the line's points as corners. A highlighter is held flat and steady: its
    /// own jitter is its wobble, so it takes no hand.
    fn ink<C: Chance>(&self, line: &Centreline, _hand: &Hand, chance: &mut C) -> Path {
        self.fill(line.points(), chance)
    }
}
