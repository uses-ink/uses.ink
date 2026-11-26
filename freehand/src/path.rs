//! SVG path data, smoothed through quadratic curves and written with two decimals.

use alloc::string::String;
use core::fmt::{self, Write};

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
        let mut d = String::new();
        let mut write = || -> fmt::Result {
            write!(d, "M{},{} Q", Fixed(first.x), Fixed(first.y))?;
            for (i, a) in outline.iter().enumerate() {
                let b = outline.get((i + 1) % outline.len()).unwrap_or(first);
                write!(d, "{},{} {},{} ", Fixed(a.x), Fixed(a.y), Fixed(a.mid(*b).x), Fixed(a.mid(*b).y))?;
            }
            d.write_str("Z")
        };
        // Writing into a `String` does not fail.
        let _written = write();
        Self(d)
    }

    /// A closed curve around corners: from the midpoint of the first two, each corner a control
    /// and the midpoint to the next an anchor. Fewer than two corners draw nothing.
    pub fn around(corners: &[Point]) -> Self {
        let (Some(first), Some(second)) = (corners.first(), corners.get(1)) else { return Self::default() };
        let n = corners.len();
        let mut d = String::new();
        let mut write = || -> fmt::Result {
            let start = first.mid(*second);
            write!(d, "M{},{} Q", Fixed(start.x), Fixed(start.y))?;
            for i in 1..=n {
                let (Some(a), Some(b)) = (corners.get(i % n), corners.get((i + 1) % n)) else { continue };
                write!(d, "{},{} {},{} ", Fixed(a.x), Fixed(a.y), Fixed(a.mid(*b).x), Fixed(a.mid(*b).y))?;
            }
            d.write_str("Z")
        };
        let _written = write();
        Self(d)
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
