//! Text safe to print: control characters shown as `\u{..}`, so a file name or a value from a
//! source cannot write escape sequences to the operator's terminal.

use std::fmt::{self, Write as _};

/// What a [`Plain`] lets through besides printable text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Keep {
    /// Line breaks and tabs, which a message lays itself out with.
    Layout,
    /// Nothing: the text stays on one line.
    Nothing,
}

/// A value shown with its control characters escaped.
#[derive(Clone, Copy, Debug)]
pub struct Plain<D> {
    shown: D,
    keep: Keep,
}

impl<D> Plain<D> {
    /// `shown`, line breaks and tabs kept: a message of several lines.
    pub const fn text(shown: D) -> Self {
        Self { shown, keep: Keep::Layout }
    }

    /// `shown` on one line: a name, a path.
    pub const fn flat(shown: D) -> Self {
        Self { shown, keep: Keep::Nothing }
    }
}

/// The writer that escapes, over the formatter it prints to.
struct Escaping<'a, 'b> {
    out: &'a mut fmt::Formatter<'b>,
    keep: Keep,
}

impl fmt::Write for Escaping<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for c in text.chars() {
            let layout = self.keep == Keep::Layout && matches!(c, '\n' | '\t');
            if c.is_control() && !layout {
                write!(self.out, "\\u{{{:x}}}", u32::from(c))?;
            } else {
                self.out.write_char(c)?;
            }
        }
        Ok(())
    }
}

impl<D: fmt::Display> fmt::Display for Plain<D> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(Escaping { out, keep: self.keep }, "{}", self.shown)
    }
}

#[cfg(test)]
mod test {
    use super::Plain;

    #[test]
    fn controls_are_escaped_and_layout_is_kept_only_in_text() {
        let name = "a\u{1b}]52;c;x\u{7}b\nc\td\u{9b}";
        assert_eq!(Plain::text(name).to_string(), "a\\u{1b}]52;c;x\\u{7}b\nc\td\\u{9b}");
        assert_eq!(Plain::flat(name).to_string(), "a\\u{1b}]52;c;x\\u{7}b\\u{a}c\\u{9}d\\u{9b}");
        assert_eq!(Plain::flat("plain text, é, 日本").to_string(), "plain text, é, 日本");
    }
}
