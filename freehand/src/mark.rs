//! Marks a theme draws on a page, each from measured geometry and a name. The name seeds every
//! random draw, so the same name and geometry give the same mark; a mark of several lines seeds
//! each with `{name}{i}` ([`Seed::and_number`]).

use alloc::vec::Vec;

use crate::centreline::{Centreline, Ellipse};
use crate::chance::{Chance, Mulberry, Seed};
use crate::hand::Hand;
use crate::highlighter::Highlighter;
use crate::path::Path;
use crate::pencil::Pencil;
use crate::point::{Point, Rectangle};

/// A quick straight line from `from` to `to`, overshooting a little: a rule or an underline.
pub fn line(from: Point, to: Point, weight: f64, name: &str) -> Path {
    seeded_line(from, to, weight, Seed::of(name))
}

/// [`line`], drawn from `seed` rather than from a name.
fn seeded_line(from: Point, to: Point, weight: f64, seed: Seed) -> Path {
    let mut chance = Mulberry::new(seed);
    let centre = Centreline::straight(from, to, &mut chance);
    Pencil::new(weight).outline(&Hand::default().trace(&centre, &mut chance))
}

/// A box drawn as four quick lines between corners that each miss their place by up to
/// `slack / 2` pixels, so the lines cross at the corners. Drawn large, it stretches as a
/// border image and keeps its wobble.
pub fn frame(around: Rectangle, slack: f64, weight: f64, name: &str) -> Vec<Path> {
    let mut chance = Mulberry::new(Seed::of(name));
    let corners = around.corners().map(|corner| {
        let dx = (chance.draw() - 0.5) * slack;
        let dy = (chance.draw() - 0.5) * slack;
        Point::new(corner.x + dx, corner.y + dy)
    });
    corners
        .iter()
        .zip(corners.iter().cycle().skip(1))
        .enumerate()
        .map(|(i, (from, to))| seeded_line(*from, *to, weight, Seed::of(name).and_number(i)))
        .collect()
}

/// An ellipse drawn in one go, its ends overlapping when it turns past once: a ring around a
/// number, or a scribbled bullet.
pub fn ring(ellipse: Ellipse, weight: f64, name: &str) -> Path {
    let mut chance = Mulberry::new(Seed::of(name));
    let centre = Centreline::ellipse(ellipse);
    Pencil::new(weight).outline(&Hand::default().trace(&centre, &mut chance))
}

/// One highlighter pass over the shape with these corners: tape, or a highlight behind words.
pub fn patch(corners: &[Point], name: &str) -> Path {
    Highlighter::default().fill(corners, &mut Mulberry::new(Seed::of(name)))
}

/// A line under a box `width` wide, at height `y` (the box's bottom, or its middle for a strike),
/// running 2 px past each side and tilted by up to 2 px.
pub fn underline(width: f64, y: f64, weight: f64, name: &str) -> Path {
    let mut chance = Mulberry::new(Seed::of(name));
    let from = Point::new(-2.0, y + chance.draw() * 2.0);
    let to = Point::new(width + 2.0, y - chance.draw() * 2.0);
    let centre = Centreline::straight(from, to, &mut chance);
    Pencil::new(weight).outline(&Hand::default().trace(&centre, &mut chance))
}

/// A bucket fill behind a box: its corners missed by up to 2 px, the whole shape moved by
/// `shift`, the way every fill of a page misses its lines the same way.
pub fn fill(behind: Rectangle, shift: Point, name: &str) -> Path {
    let mut chance = Mulberry::new(Seed::of(name));
    let corners = behind.corners().map(|corner| {
        let dx = (chance.draw() - 0.5) * 4.0;
        let dy = (chance.draw() - 0.5) * 4.0;
        Point::new(corner.x + dx + shift.x, corner.y + dy + shift.y)
    });
    Highlighter::default().fill(&corners, &mut chance)
}

/// A centreline drawn by `hand` with a pencil of `weight`. A loop that comes back near its start
/// (within a fifth of its length) would enclose its middle and fill solid, so it is drawn in two
/// overlapping arcs, the way a hand draws a circle in two goes; each arc is one path.
pub fn drawn(line: &Centreline, hand: Hand, weight: f64, name: &str) -> Vec<Path> {
    let mut chance = Mulberry::new(Seed::of(name));
    let points = line.points();
    let closed = match (points.first(), points.last()) {
        (Some(first), Some(last)) => points.len() > 8 && first.distance(*last) < line.length() * 0.2,
        _ => false,
    };
    let half = points.len() / 2;
    let arcs = match (closed, line.part(0..half + 2), line.part(half..points.len())) {
        (true, Some(one), Some(two)) => alloc::vec![one, two],
        _ => alloc::vec![line.clone()],
    };
    arcs.iter().map(|arc| Pencil::new(weight).outline(&hand.trace(arc, &mut chance))).collect()
}
