//! How help is coloured: the seam to the terminal layer.
//!
//! Help asks for a [`Face`], never for a colour. The terminal layer implements [`Paint`] with its
//! theme and its per-stream colour decision, so help is coloured exactly when the stream it is
//! written to is.

use crate::output::term::{Look, Role, Strength};

/// What a piece of help text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Face {
    /// A heading: bold.
    Heading,
    /// Anything you can type: cyan, bold.
    Literal,
}

/// Writes text in a [`Face`].
pub(crate) trait Paint {
    /// `text`, styled.
    fn paint(&self, face: Face, text: &str) -> String;
}

impl Face {
    /// The role of the terminal layer this face is written in.
    const fn role(self) -> Role {
        match self {
            Self::Heading => Role::Heading,
            Self::Literal => Role::Literal,
        }
    }
}

impl Paint for Look {
    fn paint(&self, face: Face, text: &str) -> String {
        if !self.colour {
            return text.to_owned();
        }
        let style = self.theme().style(face.role(), Strength::Strong);
        format!("{style}{text}{style:#}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_look_writes_escapes_only_when_colour_shows() {
        let coloured = Look { colour: true, ..Look::PLAIN };
        let paint = coloured.paint(Face::Heading, "x");
        assert!(paint.contains('\u{1b}'));
        assert!(paint.contains('x'));
        assert_eq!(Look::PLAIN.paint(Face::Heading, "x"), "x");
    }
}
