//! The freehand pen for the browser, as a plain wasm module: no wasm-bindgen, so no generated
//! JS. `freehand.js` beside this crate is the whole loader.
//!
//! One call at a time, in two buffers the module owns (a `Buffer`):
//!
//! 1. `input(bytes)` makes room for the call's input and returns where it starts. The caller
//!    writes the call's points there as little-endian `f64` pairs (`x, y`), then the mark's name
//!    as UTF-8 right after them.
//! 2. A mark export takes its numbers, the point count and the name's length in bytes, draws, and
//!    returns the length of its output: SVG path data, one path per line.
//! 3. `output()` returns where that output starts.
//!
//! A name that is not UTF-8 draws nothing and returns 0, as do a point count the input cannot
//! hold and a point that is not finite.
//!
//! Every mark export is one `Call` drawn through the one generic `Call::draw`: the export
//! only says which mark its numbers make and which tool draws it.

#![expect(unsafe_code, reason = "a wasm export is named with `#[unsafe(no_mangle)]`; nothing else here is unsafe")]

use std::cell::RefCell;
use std::fmt::Write as _;

use freehand::mark::{Drawn, Fill, Frame, Line, Patch, Underline};
use freehand::{Centreline, Hand, Highlighter, Mark, Mulberry, Paths, Pencil, Point, Rectangle, Seed, Tool};

thread_local! {
    static BUFFER: RefCell<Buffer> = const { RefCell::new(Buffer::new()) };
}

/// The protocol's two buffers: the input of the call being made, and the output of the last.
#[derive(Debug)]
struct Buffer {
    /// Points as `f64` pairs, then the name's bytes, padded to whole `f64`s.
    input: Vec<f64>,
    /// The last call's paths, one per line.
    output: String,
}

impl Buffer {
    const fn new() -> Self {
        Self { input: Vec::new(), output: String::new() }
    }

    /// Room for `bytes` of input, 8-byte aligned so points can be written as `f64`.
    fn room(&mut self, bytes: u32) -> *mut u8 {
        self.input.clear();
        self.input.resize((bytes as usize).div_ceil(8), 0.0);
        self.input.as_mut_ptr().cast()
    }

    /// `count` points, then the seed named by `name` bytes, as the input holds them; `None` when
    /// it cannot hold them, a point is not finite or the name is not UTF-8.
    fn read(&self, count: u32, name: u32) -> Option<(Vec<Point>, Seed)> {
        let floats_end = usize::try_from(count).ok()?.checked_mul(2)?;
        let floats = self.input.get(..floats_end)?;
        if !floats.iter().all(|f| f.is_finite()) {
            return None;
        }
        let points = floats.as_chunks::<2>().0.iter().map(|[x, y]| Point::new(*x, *y)).collect();
        let bytes: Vec<u8> = self.input.get(floats_end..)?.iter().flat_map(|f| f.to_le_bytes()).collect();
        let name = std::str::from_utf8(bytes.get(..name as usize)?).ok()?;
        Some((points, Seed::of(name)))
    }

    /// Writes `paths` as the output and returns its length.
    fn write(&mut self, paths: &Paths) -> u32 {
        self.output.clear();
        let _infallible = write!(self.output, "{paths}");
        u32::try_from(self.output.len()).unwrap_or(0)
    }
}

/// One mark export's call: the tool and hand it draws with, and what it reads from the input.
#[derive(Debug)]
struct Call<T> {
    tool: T,
    hand: Hand,
    /// How many points precede the name.
    count: u32,
    /// The name's length in bytes.
    name: u32,
}

impl<T: Tool> Call<T> {
    /// A call drawing with `tool` in the page's hand, reading a name of `name` bytes and no points.
    fn new(tool: T, name: u32) -> Self {
        Self { tool, hand: Hand::default(), count: 0, name }
    }

    /// Draws the mark `mark` makes of the input's points, seeded by the input's name, into the
    /// output, and returns the output's length; an input it cannot read draws nothing.
    fn draw<M: Mark>(self, mark: impl FnOnce(Vec<Point>) -> M) -> u32 {
        BUFFER.with_borrow_mut(|buffer| {
            let paths = match buffer.read(self.count, self.name) {
                Some((points, seed)) => mark(points).draw(&self.hand, &self.tool, &mut Mulberry::from(seed)),
                None => Paths::default(),
            };
            buffer.write(&paths)
        })
    }
}

/// Room for `bytes` of input, 8-byte aligned so points can be written as `f64`.
#[unsafe(no_mangle)]
pub extern "C" fn input(bytes: u32) -> *mut u8 {
    BUFFER.with_borrow_mut(|buffer| buffer.room(bytes))
}

/// Where the last output starts.
#[unsafe(no_mangle)]
pub extern "C" fn output() -> *const u8 {
    BUFFER.with_borrow(|buffer| buffer.output.as_ptr())
}

/// [`Line`] drawn with a pencil of `weight`.
#[unsafe(no_mangle)]
pub extern "C" fn line(from_x: f64, from_y: f64, to_x: f64, to_y: f64, weight: f64, name: u32) -> u32 {
    Call::new(Pencil::new(weight), name).draw(|_| Line { from: Point::new(from_x, from_y), to: Point::new(to_x, to_y) })
}

/// [`Underline`] drawn with a pencil of `weight`.
#[unsafe(no_mangle)]
pub extern "C" fn underline(width: f64, y: f64, weight: f64, name: u32) -> u32 {
    Call::new(Pencil::new(weight), name).draw(|_| Underline { width, y })
}

/// [`Frame`] around a `width` by `height` box at the origin, drawn with a pencil of `weight`:
/// four paths.
#[unsafe(no_mangle)]
pub extern "C" fn frame(width: f64, height: f64, slack: f64, weight: f64, name: u32) -> u32 {
    let around = Rectangle { top_left: Point::default(), bottom_right: Point::new(width, height) };
    Call::new(Pencil::new(weight), name).draw(|_| Frame { around, slack })
}

/// [`Fill`] behind a `width` by `height` box at the origin, with a highlighter.
#[unsafe(no_mangle)]
pub extern "C" fn fill(width: f64, height: f64, shift_x: f64, shift_y: f64, name: u32) -> u32 {
    let behind = Rectangle { top_left: Point::default(), bottom_right: Point::new(width, height) };
    Call::new(Highlighter::default(), name).draw(|_| Fill { behind, shift: Point::new(shift_x, shift_y) })
}

/// [`Patch`] over `count` corners, with a highlighter.
#[unsafe(no_mangle)]
pub extern "C" fn patch(count: u32, name: u32) -> u32 {
    Call { count, ..Call::new(Highlighter::default(), name) }.draw(Patch)
}

/// [`Drawn`] along `count` centreline points, with a pencil of `weight` in a hand of this
/// `wobble` and `tremor`: one path, or two for a loop.
#[unsafe(no_mangle)]
pub extern "C" fn drawn(count: u32, weight: f64, wobble: f64, tremor: f64, name: u32) -> u32 {
    let call = Call { tool: Pencil::new(weight), hand: Hand { wobble, tremor }, count, name };
    call.draw(|points| Drawn::<Centreline>(points.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

    use super::*;

    fn put(points: &[f64], name: &str) -> u32 {
        let bytes = points.len() * 8 + name.len();
        input(u32::try_from(bytes).unwrap());
        BUFFER.with_borrow_mut(|buffer| {
            let input = &mut buffer.input;
            input[..points.len()].copy_from_slice(points);
            let mut tail = Vec::new();
            tail.extend_from_slice(name.as_bytes());
            tail.resize(input.len().saturating_sub(points.len()) * 8, 0);
            for (slot, chunk) in input[points.len()..].iter_mut().zip(tail.as_chunks::<8>().0) {
                *slot = f64::from_le_bytes(*chunk);
            }
        });
        u32::try_from(name.len()).unwrap()
    }

    fn out() -> String {
        BUFFER.with_borrow(|buffer| buffer.output.clone())
    }

    fn seeded(name: &str) -> Mulberry {
        Mulberry::from(Seed::of(name))
    }

    #[test]
    fn line_matches_the_crate() {
        let name = put(&[], "rule");
        line(6.0, 6.0, 790.0, 7.0, 2.6, name);
        let rule = Line { from: Point::new(6.0, 6.0), to: Point::new(790.0, 7.0) };
        assert_eq!(out(), rule.draw(&Hand::default(), &Pencil::new(2.6), &mut seeded("rule")).to_string());
    }

    #[test]
    fn points_then_name() {
        let corners = [2.0, 4.0, 98.0, 2.0, 97.0, 27.0, 3.0, 28.0];
        let name = put(&corners, "tape");
        patch(4, name);
        let expected = [Point::new(2.0, 4.0), Point::new(98.0, 2.0), Point::new(97.0, 27.0), Point::new(3.0, 28.0)];
        let tape = Patch(expected).draw(&Hand::default(), &Highlighter::default(), &mut seeded("tape"));
        assert_eq!(out(), tape.to_string());
    }

    #[test]
    fn frames_are_four_lines() {
        let name = put(&[], "box");
        frame(100.0, 40.0, 3.0, 3.0, name);
        assert_eq!(out().lines().count(), 4);
    }

    #[test]
    fn infinite_points_draw_nothing() {
        let name = put(&[0.0, 0.0, f64::INFINITY, 0.0, 1.0, 1.0], "x");
        assert_eq!(patch(3, name), 0);
    }

    #[test]
    fn huge_lines_draw_nothing() {
        let name = put(&[], "x");
        assert_eq!(line(0.0, 0.0, 1e12, 0.0, 2.0, name), 0);
    }

    #[test]
    fn too_many_points_draw_nothing() {
        let name = put(&[1.0, 2.0], "x");
        assert_eq!(patch(9, name), 0);
    }
}
