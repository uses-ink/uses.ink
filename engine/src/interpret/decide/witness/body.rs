//! [`Body`]: a page's summary from its own text, its first paragraph cut near a sentence end.

use model::code::Summary;
use model::{Block, Evidence, Inline, Visit};

use crate::interpret::decide::Sourced;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};

/// The length a summary is cut near, in characters.
const NEAR: usize = 160;
/// A sentence end is taken from this many characters on.
const SHORTEST: usize = 80;
/// And up to this many.
const LONGEST: usize = 240;

/// The page's first paragraph, cut at a sentence end near [`NEAR`] characters; nothing for a
/// page with no paragraph or an empty one.
#[derive(Debug, Default)]
pub(crate) struct Body;

impl<G: Sourced + ?Sized> Witness<Summary, G> for Body {
    fn testify(&self, case: &Case<'_, Summary, G>, testimony: &mut Testimony<Option<String>>) {
        let Some(source) = case.ground.source() else { return };
        let first = source.read.document.blocks.iter().find_map(|block| match block {
            Block::Paragraph(inlines) => Some(inlines),
            _ => None,
        });
        let Some(inlines) = first else { return };
        let mut plain = Plain::default();
        plain.inlines(inlines);
        let text = plain.cut();
        if !text.is_empty() {
            testimony.value(Some(text), Evidence::Body { path: source.path.clone() });
        }
    }
}

/// Inlines as plain text: what a reader would read, breaks as spaces, images left out.
#[derive(Debug, Default)]
struct Plain(String);

impl Visit for Plain {
    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Code(text) | Inline::Math(text) | Inline::Keyboard(text) => self.0.push_str(text),
            Inline::Break | Inline::SoftBreak => self.0.push(' '),
            Inline::Image { .. } => {}
            other => other.walk(self),
        }
    }

    fn text(&mut self, text: &str) {
        self.0.push_str(text);
    }
}

impl Plain {
    /// The text whole when short; else up to the sentence end nearest [`NEAR`] between
    /// [`SHORTEST`] and [`LONGEST`] characters; else up to the last space before [`NEAR`],
    /// then `…`.
    fn cut(self) -> String {
        let text = self.0;
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        if chars.len() <= NEAR {
            return text;
        }
        let ends = chars.iter().enumerate().filter_map(|(at, &(byte, c))| {
            let next = chars.get(at + 1).map(|&(_, next)| next);
            (matches!(c, '.' | '!' | '?') && next.is_none_or(char::is_whitespace)).then_some((at, byte + c.len_utf8()))
        });
        let nearest = ends.filter(|(at, _)| (SHORTEST..LONGEST).contains(at)).min_by_key(|(at, _)| at.abs_diff(NEAR));
        if let Some((_, byte)) = nearest {
            return text.get(..byte).unwrap_or(&text).to_owned();
        }
        let space = chars.iter().take(NEAR).rev().find(|(_, c)| c.is_whitespace()).map(|&(byte, _)| byte);
        let byte = space.or_else(|| chars.get(NEAR).map(|&(byte, _)| byte)).unwrap_or(text.len());
        let mut cut = text.get(..byte).unwrap_or(&text).trim_end().to_owned();
        cut.push('…');
        cut
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn cut(text: &str) -> String {
        Plain(text.to_owned()).cut()
    }

    #[test]
    fn cuts_at_a_sentence_end_near_the_limit() {
        assert_eq!(cut("Short."), "Short.");
        let long = format!("{}. {}. {}", "a".repeat(100), "b".repeat(50), "c".repeat(100));
        assert_eq!(cut(&long), format!("{}. {}.", "a".repeat(100), "b".repeat(50)));
        let words = "word ".repeat(60);
        let cut = cut(&words);
        assert!(cut.ends_with("word…") && cut.chars().count() <= NEAR + 1, "{cut}");
    }
}
