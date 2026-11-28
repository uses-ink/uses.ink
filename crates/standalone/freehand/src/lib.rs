//! Hand-drawn strokes, the same on every target.
//!
//! A drawing starts as clean geometry: a [`Centreline`] (a segment, a curve, an ellipse). A
//! [`Hand`] traces it with wobble, a slight bow and tremor, its random draws from a [`Chance`]
//! seeded by a [`Seed`], and gives each point a pressure. A [`Tool`] turns what the hand draws
//! into ink: a [`Pencil`] into the filled outline of its line, a port of perfect-freehand's
//! `getStroke`; a [`Highlighter`] into one smooth marker pass that misses its lines. Ink is a
//! [`Path`], written as SVG path data. The [`mark`]s a theme draws (rules, frames, rings,
//! highlights) are data, each a [`Mark`] that draws itself with any tool into [`Paths`].
//!
//! The outline is a port of [perfect-freehand](https://github.com/steveruizok/perfect-freehand)
//! 1.2.3 by Steve Ruiz, MIT licensed (`LICENSE-PERFECT-FREEHAND`).
//!
//! Every number goes through `libm` and every random draw through [`Mulberry`], so a drawing is
//! a function of its seed and its geometry: the same bytes natively and in wasm. A line that
//! would need more than [`MOST`] points, or has a non-finite coordinate, draws nothing.
//!
//! ```
//! use freehand::mark::Line;
//! use freehand::{Hand, Mark, Mulberry, Pencil, Point, Seed};
//!
//! let rule = Line { from: Point::new(0.0, 0.0), to: Point::new(40.0, 0.0) };
//! let ink = rule.draw(&Hand::default(), &Pencil::new(3.0), &mut Mulberry::from(Seed::of("rule")));
//! assert!(ink.to_string().starts_with('M'));
//! ```

#![no_std]

extern crate alloc;

mod centreline;
mod chance;
mod hand;
pub mod mark;
mod path;
mod point;
mod tool;

pub use centreline::{Centreline, Ellipse, MOST};
pub use chance::{Chance, Constant, Mulberry, Seed};
pub use hand::Hand;
pub use path::{Path, Paths};
pub use point::{Point, Pressured, Rectangle};
pub use tool::{Cap, Easing, End, Highlighter, Outline, Pencil, Taper};

/// Something a hand draws with: it turns a centreline, as the hand draws it, into ink.
pub trait Tool {
    /// The ink of `hand` drawing `line` with this tool, every random draw from `chance`.
    fn ink<C: Chance>(&self, line: &Centreline, hand: &Hand, chance: &mut C) -> Path;
}

/// A drawing made of its geometry: a rule, a frame, a ring, a highlight ([`mark`]).
pub trait Mark {
    /// The paths of this mark as `hand` draws it with `tool`, every random draw from `chance`.
    fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths;
}
