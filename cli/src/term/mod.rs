//! The one place that decides how output looks: colour roles, glyphs, width and terminal
//! detection, the layout types that pad and cut, and the writer that turns a [`Doc`] into bytes
//! (decision 0061, `docs/cli.md` sections 5, 6 and 16).
//!
//! A block of output is a typed value that implements [`Render`]: it pushes lines of roles and
//! text into a [`Doc`] and never writes an escape code. The [`Writer`] paints the [`Doc`] through
//! the [`Theme`], or leaves it plain. Nothing here reads the process: [`Surroundings`] is what the
//! caller says the process is, so a test sets it and a golden never flips with the machine.

mod doc;
mod glyph;
mod layout;
mod look;
mod meter;
mod paint;
mod role;

pub(crate) use doc::{Doc, Line, Piece, Render, Safe};
pub(crate) use glyph::Glyph;
#[cfg(test)]
pub(crate) use glyph::Glyphs;
pub(crate) use layout::{Answer, Cut, Flex, Gutter, Rail, Record, Styled, Table, Tree, Trees, Verbatim};
pub use look::Surroundings;
pub(crate) use look::{Asked, Look, Terminal, Width};
#[cfg(test)]
pub(crate) use meter::Tick;
pub(crate) use meter::{Beat, Draw, Elapsed, Meter, Pacing, Phase, Show};
pub(crate) use paint::Writer;
#[cfg(test)]
pub(crate) use role::Theme;
pub(crate) use role::{Role, Strength};
