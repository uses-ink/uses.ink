//! The marks a theme draws on a page. Each is the geometry of one drawing, measured from the thing
//! it marks, and draws itself as a [`Mark`] with any [`Tool`] in any [`Hand`]. Seed its chance
//! with the mark's name (`Mulberry::from(Seed::of(name))`) and the same name and geometry draw the
//! same mark; a mark of several lines draws line `i` from the chance's fork `i`, so from the seed
//! `{name}{i}`.
//!
//! ```
//! use freehand::mark::Frame;
//! use freehand::{Hand, Mark, Mulberry, Pencil, Point, Rectangle, Seed};
//!
//! let around = Rectangle { top_left: Point::new(0.0, 0.0), bottom_right: Point::new(100.0, 40.0) };
//! let frame = Frame { around, slack: 3.0 };
//! let paths = frame.draw(&Hand::default(), &Pencil::new(2.6), &mut Mulberry::from(Seed::of("box")));
//! assert_eq!(paths.len(), 4);
//! ```

use core::borrow::Borrow;

use crate::centreline::{Centreline, Ellipse};
use crate::chance::Chance;
use crate::hand::Hand;
use crate::path::Paths;
use crate::point::{Point, Rectangle};
use crate::{Mark, Tool};

/// A quick straight line from `from` to `to`, overshooting a little: a rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// Where the line is meant to start.
    pub from: Point,
    /// Where it is meant to end.
    pub to: Point,
}

impl Mark for Line {
    fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths {
        let centre = Centreline::straight(self.from, self.to, chance);
        tool.ink(&centre, hand, chance).into()
    }
}

/// A line under a box `width` wide, at height `y` (the box's bottom, or its middle for a strike),
/// running 2 px past each side and tilted by up to 2 px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Underline {
    /// The width of the box underlined.
    pub width: f64,
    /// The height the line runs at.
    pub y: f64,
}

impl Mark for Underline {
    fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths {
        let from = Point::new(-2.0, self.y + chance.draw() * 2.0);
        let to = Point::new(self.width + 2.0, self.y - chance.draw() * 2.0);
        Line { from, to }.draw(hand, tool, chance)
    }
}

/// A box drawn as four quick lines between corners that each miss their place by up to
/// `slack / 2` pixels, so the lines cross at the corners. Drawn large, it stretches as a border
/// image and keeps its wobble.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The box it is drawn around.
    pub around: Rectangle,
    /// How far, across both directions, each corner may miss.
    pub slack: f64,
}

impl Mark for Frame {
    /// Four paths, clockwise from the top; line `i` draws from `chance.fork(i)`.
    fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths {
        let corners = self.around.jittered(self.slack, chance);
        corners
            .iter()
            .zip(corners.iter().cycle().skip(1))
            .enumerate()
            .flat_map(|(i, (from, to))| Line { from: *from, to: *to }.draw(hand, tool, &mut chance.fork(i)))
            .collect()
    }
}

/// An ellipse drawn in one go, its ends overlapping when it turns past once: a ring around a
/// number, or a scribbled bullet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring(pub Ellipse);

impl Mark for Ring {
    fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths {
        tool.ink(&Centreline::from(self.0), hand, chance).into()
    }
}

/// A bucket fill behind a box: its corners missed by up to 2 px, the whole shape moved by `shift`,
/// the way every fill of a page misses its lines the same way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    /// The box it fills.
    pub behind: Rectangle,
    /// How far the whole fill misses the box.
    pub shift: Point,
}

impl Mark for Fill {
    fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths {
        let corners = self.behind.jittered(4.0, chance).map(|corner| corner + self.shift);
        tool.ink(&corners.into_iter().collect(), hand, chance).into()
    }
}

/// One pass over the shape with these corners, in any container of points: tape, or a highlight
/// behind words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch<P>(pub P);

impl<P: AsRef<[Point]>> Mark for Patch<P> {
    fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths {
        tool.ink(&self.0.as_ref().iter().copied().collect(), hand, chance).into()
    }
}

/// Any centreline, owned or borrowed, drawn as it is. A loop that comes back near its start
/// (within a fifth of its length) would enclose its middle and fill solid, so it is drawn in two
/// overlapping arcs, the way a hand draws a circle in two goes; each arc is one path.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn<L>(pub L);

impl<L: Borrow<Centreline>> Mark for Drawn<L> {
    /// One path, or two for a loop, both drawing from `chance` in turn.
    fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths {
        let line = self.0.borrow();
        let points = line.points();
        let closed = match (points.first(), points.last()) {
            (Some(first), Some(last)) => points.len() > 8 && first.distance(*last) < line.length() * 0.2,
            _ => false,
        };
        let half = points.len() / 2;
        match (closed, line.part(0..half + 2), line.part(half..points.len())) {
            (true, Some(one), Some(two)) => [one, two].iter().map(|arc| tool.ink(arc, hand, chance)).collect(),
            _ => tool.ink(line, hand, chance).into(),
        }
    }
}
