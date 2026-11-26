//! A highlighter pass: an outline jittered, then smoothed, so a fill reads as one marker stroke
//! that misses its lines.

use alloc::vec::Vec;

use crate::chance::Chance;
use crate::number::round;
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
    /// One pass over the shape with these corners.
    pub fn fill(&self, corners: &[Point], chance: &mut impl Chance) -> Path {
        let mut points = Vec::new();
        for (i, from) in corners.iter().enumerate() {
            let Some(to) = corners.get((i + 1) % corners.len()) else { continue };
            let count = libm::fmax(1.0, round(libm::hypot(to.x - from.x, to.y - from.y) / self.spacing));
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
