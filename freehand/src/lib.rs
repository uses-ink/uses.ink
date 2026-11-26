//! Hand-drawn strokes, the same on every target.
//!
//! A drawing starts as clean geometry: a [`Centreline`] (a segment, a curve, an ellipse). A
//! [`Hand`] traces it with wobble, a slight bow and tremor, seeded by a [`Seed`], and gives each
//! point a pressure. A [`Pencil`] turns the traced line into the filled outline of its ink, a
//! port of perfect-freehand's `getStroke`; a [`Highlighter`] turns corners into one smooth
//! marker pass that misses its lines. Both give a [`Path`], written as SVG path data. The
//! [`mark`]s a theme draws (rules, frames, rings, highlights) are built from these.
//!
//! The outline is a port of [perfect-freehand](https://github.com/steveruizok/perfect-freehand)
//! 1.2.3 by Steve Ruiz, MIT licensed (`LICENSE-PERFECT-FREEHAND`).
//!
//! Every number goes through `libm` and every random draw through [`Mulberry`], so a drawing is
//! a function of its seed and its geometry: the same bytes natively and in wasm. A line that
//! would need more than [`MOST`] points, or has a non-finite coordinate, draws nothing.
//!
//! ```
//! use freehand::{Centreline, Hand, Mulberry, Pencil, Point, Seed};
//!
//! let line = Centreline::segment(Point::new(0.0, 0.0), Point::new(40.0, 0.0));
//! let mut chance = Mulberry::new(Seed::of("rule"));
//! let traced = Hand::default().trace(&line, &mut chance);
//! let ink = Pencil::new(3.0).outline(&traced);
//! assert!(ink.to_string().starts_with('M'));
//! ```

#![no_std]

extern crate alloc;

mod centreline;
mod chance;
mod hand;
mod highlighter;
pub mod mark;
mod number;
mod outline;
mod path;
mod pencil;
mod point;

pub use centreline::{Centreline, MOST};
pub use chance::{Chance, Constant, Mulberry, Seed};
pub use hand::Hand;
pub use highlighter::Highlighter;
pub use outline::{Cap, Easing, End, Outline, Taper};
pub use path::Path;
pub use pencil::Pencil;
pub use point::{Point, Pressured};
