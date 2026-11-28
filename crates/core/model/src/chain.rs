//! An error with the causes below it, the one place a chain becomes text.

use std::error::Error;
use std::fmt;

/// An error and its causes, each after a separator (`: ` unless [`Chain::joined`] says another).
/// Where a cause becomes text exactly once, at an edge, this says so; [`Chain::causes`] gives the
/// causes themselves for a caller that lists them.
#[derive(Clone, Copy, Debug)]
pub struct Chain<'a> {
    error: &'a (dyn Error + 'a),
    separator: &'a str,
}

impl<'a> Chain<'a> {
    /// The separator between an error and each cause below it.
    pub const SEPARATOR: &'static str = ": ";

    /// `error` and its causes, joined by [`Chain::SEPARATOR`].
    #[must_use]
    pub const fn new(error: &'a (dyn Error + 'a)) -> Self {
        Self { error, separator: Self::SEPARATOR }
    }

    /// The same chain, joined by `separator`.
    #[must_use]
    pub const fn joined(self, separator: &'a str) -> Self {
        Self { separator, ..self }
    }

    /// The causes below the error, nearest first; the error itself is not among them.
    pub fn causes(&self) -> impl Iterator<Item = &'a (dyn Error + 'static)> + use<'a> {
        let error: &'a (dyn Error + 'a) = self.error;
        std::iter::successors(error.source(), |cause: &&'a (dyn Error + 'static)| Error::source(*cause))
    }
}

impl fmt::Display for Chain<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.error)?;
        for cause in self.causes() {
            write!(out, "{}{cause}", self.separator)?;
        }
        Ok(())
    }
}
