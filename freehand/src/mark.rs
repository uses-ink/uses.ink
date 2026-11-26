//! Marks a theme draws on a page, each from measured geometry and a name. The name seeds every
//! random draw, so the same name and geometry give the same mark; a mark of several lines seeds
//! each with `{name}{i}`.

use alloc::format;
use alloc::vec::Vec;

use crate::centreline::Centreline;
use crate::chance::{Chance, Mulberry, Seed};
use crate::hand::Hand;
use crate::highlighter::Highlighter;
use crate::path::Path;
use crate::pencil::Pencil;
use crate::point::Point;

/// A quick straight line from `from` to `to`, overshooting a little: a rule or an underline.
pub fn line(from: Point, to: Point, weight: f64, name: &str) -> Path {
    let mut chance = Mulberry::new(Seed::of(name));
    let centre = Centreline::straight(from, to, &mut chance);
    Pencil::new(weight).outline(&Hand::default().trace(&centre, &mut chance))
}

/// A box drawn as four quick lines between corners that each miss their place by up to
/// `slack / 2` pixels, so the lines cross at the corners. Drawn large, it stretches as a
/// border image and keeps its wobble.
pub fn frame(top_left: Point, bottom_right: Point, slack: f64, weight: f64, name: &str) -> Vec<Path> {
    let mut chance = Mulberry::new(Seed::of(name));
    let mut miss = |x: f64, y: f64| {
        let dx = (chance.draw() - 0.5) * slack;
        let dy = (chance.draw() - 0.5) * slack;
        Point::new(x + dx, y + dy)
    };
    let corners = [
        miss(top_left.x, top_left.y),
        miss(bottom_right.x, top_left.y),
        miss(bottom_right.x, bottom_right.y),
        miss(top_left.x, bottom_right.y),
    ];
    corners
        .iter()
        .zip(corners.iter().cycle().skip(1))
        .enumerate()
        .map(|(i, (from, to))| line(*from, *to, weight, &format!("{name}{i}")))
        .collect()
}

/// An ellipse drawn in one go of `count` points from angle `from`, going `turns` times round,
/// so its ends overlap: a ring around a number, or a scribbled bullet.
pub fn ring(centre: Point, radius: Point, from: f64, turns: f64, count: u32, weight: f64, name: &str) -> Path {
    let mut chance = Mulberry::new(Seed::of(name));
    let centre = Centreline::ellipse(centre, radius, from, turns, count);
    Pencil::new(weight).outline(&Hand::default().trace(&centre, &mut chance))
}

/// One highlighter pass over the shape with these corners: tape, or a highlight behind words.
pub fn patch(corners: &[Point], name: &str) -> Path {
    Highlighter::default().fill(corners, &mut Mulberry::new(Seed::of(name)))
}
