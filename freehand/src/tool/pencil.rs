//! An MS Paint pencil: thin, nearly one width, barely smoothed. A mouse has no pressure.

use super::outline::{Cap, End, Outline};
use crate::Tool;
use crate::centreline::Centreline;
use crate::chance::Chance;
use crate::hand::Hand;
use crate::path::Path;
use crate::point::Pressured;

/// A pencil of one weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pencil {
    /// The width of its line, in pixels.
    pub weight: f64,
}

impl Pencil {
    /// A pencil drawing lines `weight` pixels wide.
    pub const fn new(weight: f64) -> Self {
        Self { weight }
    }

    /// The options it gives perfect-freehand.
    pub fn options(&self) -> Outline {
        Outline {
            size: self.weight,
            thinning: 0.02,
            smoothing: 0.25,
            streamline: 0.12,
            simulate_pressure: false,
            start: End::first(Cap::Round),
            end: End::last(Cap::Round),
            ..Outline::default()
        }
    }

    /// The ink of a traced line, as a filled outline.
    pub fn outline(&self, traced: &[Pressured]) -> Path {
        Path::through(&self.options().around(traced))
    }
}

impl Tool for Pencil {
    /// The outline of the line as `hand` traces it.
    fn ink<C: Chance>(&self, line: &Centreline, hand: &Hand, chance: &mut C) -> Path {
        self.outline(&hand.trace(line, chance))
    }
}
