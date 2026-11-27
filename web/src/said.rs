//! Showing a value that has words of its own (`Display`) inside a page.

use std::fmt;

use unspliced::{Halt, Render, Sink};

/// A value written as its `Display` says it, escaped for where it lands, with its control
/// characters shown as `\u{..}` (`diag::Plain`): a file name or a value from a source never
/// reaches a page, a log or a log reader's terminal as an escape sequence.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Said<T>(pub(crate) T);

impl<T: fmt::Display> Render for Said<T> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        out.text(&diag::Plain::flat(&self.0).to_string())
    }
}

impl<T: fmt::Display> fmt::Display for Said<T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(out)
    }
}
