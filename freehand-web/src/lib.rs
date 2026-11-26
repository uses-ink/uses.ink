//! The freehand pen for the browser, as a plain wasm module: no wasm-bindgen, so no generated
//! JS. `freehand.js` beside this crate is the whole loader.
//!
//! One call at a time, in two buffers the module owns:
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

#![expect(unsafe_code, reason = "a wasm export is named with `#[unsafe(no_mangle)]`; nothing else here is unsafe")]

use std::cell::RefCell;
use std::fmt::Write as _;

use freehand::{Centreline, Hand, Path, Point, mark};

thread_local! {
    static INPUT: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Room for `bytes` of input, 8-byte aligned so points can be written as `f64`.
#[unsafe(no_mangle)]
pub extern "C" fn input(bytes: u32) -> *mut u8 {
    INPUT.with_borrow_mut(|input| {
        input.clear();
        input.resize((bytes as usize).div_ceil(8), 0.0);
        input.as_mut_ptr().cast()
    })
}

/// Where the last output starts.
#[unsafe(no_mangle)]
pub extern "C" fn output() -> *const u8 {
    OUTPUT.with_borrow(|out| out.as_ptr())
}

/// `count` points, then a name of `name` bytes, as `input` was given them.
fn read(count: u32, name: u32) -> Option<(Vec<Point>, String)> {
    INPUT.with_borrow(|input| {
        let floats_end = usize::try_from(count).ok()?.checked_mul(2)?;
        let floats = input.get(..floats_end)?;
        if !floats.iter().all(|f| f.is_finite()) {
            return None;
        }
        let points = floats.as_chunks::<2>().0.iter().map(|[x, y]| Point::new(*x, *y)).collect();
        let bytes: Vec<u8> = input.get(floats_end..)?.iter().flat_map(|f| f.to_le_bytes()).collect();
        let name = String::from_utf8(bytes.get(..name as usize)?.to_vec()).ok()?;
        Some((points, name))
    })
}

/// Writes `paths` as the output, one per line, and returns its length.
fn written(paths: &[Path]) -> u32 {
    OUTPUT.with_borrow_mut(|out| {
        out.clear();
        for (i, path) in paths.iter().enumerate() {
            let _written = write!(out, "{}{path}", if i == 0 { "" } else { "\n" });
        }
        u32::try_from(out.len()).unwrap_or(0)
    })
}

fn draw(count: u32, name: u32, mark: impl FnOnce(&[Point], &str) -> Vec<Path>) -> u32 {
    match read(count, name) {
        Some((points, name)) => written(&mark(&points, &name)),
        None => written(&[]),
    }
}

/// `mark::line`.
#[unsafe(no_mangle)]
pub extern "C" fn line(from_x: f64, from_y: f64, to_x: f64, to_y: f64, weight: f64, name: u32) -> u32 {
    draw(0, name, |_, name| vec![mark::line(Point::new(from_x, from_y), Point::new(to_x, to_y), weight, name)])
}

/// `mark::underline`.
#[unsafe(no_mangle)]
pub extern "C" fn underline(width: f64, y: f64, weight: f64, name: u32) -> u32 {
    draw(0, name, |_, name| vec![mark::underline(width, y, weight, name)])
}

/// `mark::frame` around a `width` by `height` box at the origin: four paths.
#[unsafe(no_mangle)]
pub extern "C" fn frame(width: f64, height: f64, slack: f64, weight: f64, name: u32) -> u32 {
    draw(0, name, |_, name| mark::frame(Point::new(0.0, 0.0), Point::new(width, height), slack, weight, name))
}

/// `mark::fill` behind a `width` by `height` box at the origin.
#[unsafe(no_mangle)]
pub extern "C" fn fill(width: f64, height: f64, shift_x: f64, shift_y: f64, name: u32) -> u32 {
    draw(0, name, |_, name| {
        vec![mark::fill(Point::new(0.0, 0.0), Point::new(width, height), Point::new(shift_x, shift_y), name)]
    })
}

/// `mark::patch` over `count` corners.
#[unsafe(no_mangle)]
pub extern "C" fn patch(count: u32, name: u32) -> u32 {
    draw(count, name, |corners, name| vec![mark::patch(corners, name)])
}

/// `mark::drawn` along `count` centreline points: one path, or two for a loop.
#[unsafe(no_mangle)]
pub extern "C" fn drawn(count: u32, weight: f64, wobble: f64, tremor: f64, name: u32) -> u32 {
    draw(count, name, |points, name| {
        mark::drawn(&Centreline::through(points.iter().copied()), Hand { wobble, tremor }, weight, name)
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

    use super::*;

    fn put(points: &[f64], name: &str) -> u32 {
        let bytes = points.len() * 8 + name.len();
        input(u32::try_from(bytes).unwrap());
        INPUT.with_borrow_mut(|input| {
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
        OUTPUT.with_borrow(Clone::clone)
    }

    #[test]
    fn line_matches_the_crate() {
        let name = put(&[], "rule");
        line(6.0, 6.0, 790.0, 7.0, 2.6, name);
        assert_eq!(out(), mark::line(Point::new(6.0, 6.0), Point::new(790.0, 7.0), 2.6, "rule").to_string());
    }

    #[test]
    fn points_then_name() {
        let corners = [2.0, 4.0, 98.0, 2.0, 97.0, 27.0, 3.0, 28.0];
        let name = put(&corners, "tape");
        patch(4, name);
        let expected = [Point::new(2.0, 4.0), Point::new(98.0, 2.0), Point::new(97.0, 27.0), Point::new(3.0, 28.0)];
        assert_eq!(out(), mark::patch(&expected, "tape").to_string());
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
