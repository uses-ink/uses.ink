use freehand::mark::{Drawn, Fill, Frame, Line, Patch, Ring, Underline};
use freehand::{Chance, Hand, Highlighter, Mark, Mulberry, Paths, Pencil, Point, Seed};

use super::State;
use crate::bind::plugin::ink::host::pen;

/// A mark drawn with whichever tool was asked for.
struct Drawing<'m, M>(&'m M);

impl<M: Mark> Drawing<'_, M> {
    fn with(&self, tool: pen::Tool, hand: &Hand, chance: &mut impl Chance) -> Paths {
        match tool {
            pen::Tool::Pencil(weight) => self.0.draw(hand, &Pencil::new(weight), chance),
            pen::Tool::Highlighter(h) => {
                self.0.draw(hand, &Highlighter { spacing: h.spacing, jitter: h.jitter }, chance)
            }
        }
    }
}

impl pen::Host for State {
    fn draw(&mut self, mark: pen::Mark, tool: pen::Tool, hand: pen::Hand, seed: String) -> Vec<String> {
        let hand = Hand::from(hand);
        let mut chance = Mulberry::from(Seed::of(&seed));
        let points = |points: Vec<pen::Point>| points.into_iter().map(Point::from).collect::<Vec<_>>();
        let paths = match mark {
            pen::Mark::Line(line) => {
                Drawing(&Line { from: line.start.into(), to: line.end.into() }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Underline(u) => Drawing(&Underline { width: u.width, y: u.y }).with(tool, &hand, &mut chance),
            pen::Mark::Frame(f) => {
                Drawing(&Frame { around: f.around.into(), slack: f.slack }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Ring(ellipse) => Drawing(&Ring(ellipse.into())).with(tool, &hand, &mut chance),
            pen::Mark::Fill(f) => {
                Drawing(&Fill { behind: f.behind.into(), shift: f.shift.into() }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Patch(corners) => Drawing(&Patch(points(corners))).with(tool, &hand, &mut chance),
            pen::Mark::Drawn(line) => Drawing(&Drawn(points(line).into_iter().collect::<freehand::Centreline>())).with(
                tool,
                &hand,
                &mut chance,
            ),
        };
        paths.iter().map(|path| path.as_str().to_owned()).collect()
    }
}
