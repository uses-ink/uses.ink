//! SVG path data, smoothed through quadratic curves and written with two decimals.

use alloc::string::String;
use core::fmt;

use crate::number::Fixed;
use crate::point::Point;

/// The `d` of a filled SVG path.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Path(String);

impl Path {
    /// A closed curve through an outline: from its first point, each point a control and the
    /// midpoint to the next an anchor. Fewer than four points draw nothing.
    pub fn through(outline: &[Point]) -> Self {
        let Some(first) = outline.first().filter(|_| outline.len() >= 4) else { return Self::default() };
        let mut d = Self::with_room(outline.len());
        d.start(*first);
        for (i, a) in outline.iter().enumerate() {
            d.curve(*a, a.mid(*outline.get((i + 1) % outline.len()).unwrap_or(first)));
        }
        d.0.push('Z');
        d
    }

    /// A closed curve around corners: from the midpoint of the first two, each corner a control
    /// and the midpoint to the next an anchor. Fewer than two corners draw nothing.
    pub fn around(corners: &[Point]) -> Self {
        let (Some(first), Some(second)) = (corners.first(), corners.get(1)) else { return Self::default() };
        let n = corners.len();
        let mut d = Self::with_room(n);
        d.start(first.mid(*second));
        for i in 1..=n {
            let (Some(a), Some(b)) = (corners.get(i % n), corners.get((i + 1) % n)) else { continue };
            d.curve(*a, a.mid(*b));
        }
        d.0.push('Z');
        d
    }

    /// Room for `points` curves: each is four numbers of about seven characters.
    fn with_room(points: usize) -> Self {
        Self(String::with_capacity(points.saturating_mul(30).saturating_add(24)))
    }

    /// `M x,y Q`.
    fn start(&mut self, at: Point) {
        self.0.push('M');
        self.pair(at);
        self.0.push_str(" Q");
    }

    /// `cx,cy x,y ` after the `Q`.
    fn curve(&mut self, control: Point, to: Point) {
        self.pair(control);
        self.0.push(' ');
        self.pair(to);
        self.0.push(' ');
    }

    fn pair(&mut self, at: Point) {
        Fixed(at.x).push_to(&mut self.0);
        self.0.push(',');
        Fixed(at.y).push_to(&mut self.0);
    }

    /// Whether it draws nothing.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The path data.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Path {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}
