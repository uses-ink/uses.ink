//! An MS Paint pencil: thin, nearly one width, barely smoothed. A mouse has no pressure.

use crate::outline::{Cap, End, Outline};
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
