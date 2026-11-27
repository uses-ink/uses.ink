//! The pen's marks on the dashboard, drawn at render time by the shared pen (`freehand`): the
//! same name always draws the same line, so a page is a function of its view model.

use freehand::mark::Line;
use freehand::{Hand, Mark, Mulberry, Pencil, Point, Seed};
use unspliced::{Halt, Render, Sink, html};

use crate::style::look;

/// What colour a rule is drawn in: the pencil's black, or the red note pen where uses.ink speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Ink {
    /// The black pencil.
    Pencil,
    /// The red note pen.
    Pen,
}

/// A rule drawn under a heading. Its seed is the heading's own words, so two pages with the
/// same heading wobble the same way.
#[derive(Clone, Copy, Debug)]
pub struct Rule<'a> {
    /// What the rule is drawn under; the seed of its wobble.
    pub under: &'a str,
    /// The pen it is drawn with.
    pub ink: Ink,
}

impl Rule<'_> {
    /// The width of the drawing, in the units of its `viewBox`.
    const WIDTH: f64 = 600.0;
}

impl Render for Rule<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let line = Line { from: Point::new(2.0, 7.0), to: Point::new(Self::WIDTH - 2.0, 7.0) };
        let ink = line.draw(&Hand::default(), &Pencil::new(3.0), &mut Mulberry::from(Seed::of(self.under)));
        let path = ink.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ");
        match self.ink {
            Ink::Pen => html! {
                svg.look::RULE.look::RED viewBox="0 0 600 14" preserveAspectRatio="none" aria-hidden="true" {
                    path d=(&path);
                }
            }
            .render(out),
            _ => html! {
                svg.look::RULE viewBox="0 0 600 14" preserveAspectRatio="none" aria-hidden="true" {
                    path d=(&path);
                }
            }
            .render(out),
        }
    }
}
