//! SVG path data, smoothed through quadratic curves and written in tenths of a pixel.

use alloc::string::String;
use core::fmt;

use crate::number::Tenths;
use crate::point::Point;

/// The `d` of a filled SVG path.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Path(String);

impl Path {
    /// A closed curve through an outline: from its first point, each point a control and the
    /// midpoint to the next an anchor. Fewer than four points draw nothing.
    pub fn through(outline: &[Point]) -> Self {
        let n = outline.len();
        let (Some(first), true) = (outline.first(), n >= 4) else { return Self::default() };
        Self::closed(*first, n, |k| outline.get(k % n).copied().unwrap_or(*first))
    }

    /// A closed curve around corners: from the midpoint of the first two, each corner a control
    /// and the midpoint to the next an anchor. Fewer than two corners draw nothing.
    pub fn around(corners: &[Point]) -> Self {
        let n = corners.len();
        let (Some(first), Some(second)) = (corners.first(), corners.get(1)) else { return Self::default() };
        Self::closed(first.mid(*second), n, |k| corners.get((k + 1) % n).copied().unwrap_or(*first))
    }

    /// `count` quadratic curves from `start`: curve `k` has control `control(k)` and ends at the
    /// midpoint of `control(k)` and `control(k + 1)`.
    ///
    /// Written as one `Q` and then a `T` chain: a `T` takes its control as the last one mirrored
    /// through the last anchor, and mirroring a control through the midpoint to the next gives
    /// exactly the next. Rounding would make those mirrored controls drift, so each anchor is
    /// rounded to put the next control on its true point instead: every control and anchor
    /// written or implied stays within a tenth of a pixel on each axis. All of it in whole tenths, so
    /// it is exact. A non-finite or huge coordinate draws nothing.
    fn closed(start: Point, count: usize, control: impl Fn(usize) -> Point) -> Self {
        let mut d = Self(String::with_capacity(count.saturating_mul(10).saturating_add(24)));
        let written = d.write_closed(start, count, &control);
        if written.is_none() {
            return Self::default();
        }
        d
    }

    fn write_closed(&mut self, start: Point, count: usize, control: &impl Fn(usize) -> Point) -> Option<()> {
        self.0.push('M');
        self.pair(Spot::of(start)?);
        let mut implied = Spot::of(control(0))?;
        self.0.push_str(" Q");
        self.pair(implied);
        for k in 0..count {
            let next = control(k + 1);
            let anchor = if k + 1 == count {
                Spot::of(control(k).mid(next))?
            } else {
                // the anchor that mirrors this control onto the next one
                Spot::steering(implied, next)?
            };
            self.0.push(' ');
            if k == 1 {
                self.0.push('T');
            }
            self.pair(anchor);
            implied = anchor.mirror(implied);
        }
        self.0.push('Z');
        Some(())
    }

    fn pair(&mut self, spot: Spot) {
        spot.x.push_to(&mut self.0);
        self.0.push(',');
        spot.y.push_to(&mut self.0);
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

/// A point in whole tenths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Spot {
    x: Tenths,
    y: Tenths,
}

impl Spot {
    fn of(at: Point) -> Option<Self> {
        Some(Self { x: Tenths::of(at.x)?, y: Tenths::of(at.y)? })
    }

    /// The anchor halfway between `control` and the true point `next`, so that mirroring
    /// `control` through it lands on `next`.
    fn steering(control: Self, next: Point) -> Option<Self> {
        // a non-finite or huge `next` draws nothing, as any other coordinate does
        Self::of(next)?;
        #[expect(clippy::cast_precision_loss, reason = "tenths below 2^53, so exact")]
        let half = |c: Tenths, t: f64| Tenths::nearest(f64::midpoint(c.0 as f64, t));
        Some(Self { x: half(control.x, next.x * 10.0), y: half(control.y, next.y * 10.0) })
    }

    /// `control` mirrored through this anchor: the control a `T` after it uses.
    fn mirror(self, control: Self) -> Self {
        Self { x: Tenths(2 * self.x.0 - control.x.0), y: Tenths(2 * self.y.0 - control.y.0) }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

    use alloc::borrow::ToOwned;
    use alloc::vec::Vec;

    use super::*;

    /// Reads `M s Q c a T a a .. Z` back into its start, controls (written or implied) and
    /// anchors.
    fn read(d: &str) -> (Point, Vec<Point>, Vec<Point>) {
        let body = d.strip_prefix('M').unwrap().strip_suffix('Z').unwrap();
        let body = body.replace(" Q", " ").replace(" T", " ");
        let points: Vec<Point> = body
            .split(' ')
            .map(|pair| {
                let (x, y) = pair.split_once(',').unwrap();
                Point::new(x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        let (start, control, rest) = (points[0], points[1], &points[2..]);
        let mut controls = alloc::vec![control];
        for before in &rest[..rest.len() - 1] {
            let last = controls[controls.len() - 1];
            controls.push(Point::new(2.0 * before.x - last.x, 2.0 * before.y - last.y));
        }
        (start, controls, rest.to_vec())
    }

    #[test]
    fn controls_never_drift() {
        // a long wobbly outline, the kind that drifts most
        let outline: Vec<Point> = (0..400)
            .map(|i| {
                let t = f64::from(i) * 0.37;
                Point::new(f64::from(i) * 1.73 + libm::sin(t) * 3.1, 40.0 + libm::cos(t * 1.9) * 7.3)
            })
            .collect();
        let d = Path::through(&outline).as_str().to_owned();
        let (start, controls, anchors) = read(&d);
        // per axis: an anchor rounds by at most .05, so a mirrored control misses by at most
        // twice that, and an anchor sits within .05 plus half its control's miss
        let near = |a: Point, b: Point, limit: f64| libm::fabs(a.x - b.x) <= limit && libm::fabs(a.y - b.y) <= limit;
        assert!(near(start, outline[0], 0.05 + 1e-9));
        assert_eq!(controls.len(), outline.len());
        for (k, control) in controls.iter().enumerate() {
            assert!(near(*control, outline[k], 0.1 + 1e-9), "control {k}: {control:?} vs {:?}", outline[k]);
        }
        for (k, anchor) in anchors.iter().enumerate() {
            let mid = outline[k].mid(outline[(k + 1) % outline.len()]);
            assert!(near(*anchor, mid, 0.1 + 1e-9), "anchor {k}: {anchor:?} vs {mid:?}");
        }
    }

    #[test]
    fn non_finite_draws_nothing() {
        let outline = [Point::new(0.0, 0.0), Point::new(f64::NAN, 1.0), Point::new(2.0, 2.0), Point::new(3.0, 0.0)];
        assert!(Path::through(&outline).is_empty());
    }
}
