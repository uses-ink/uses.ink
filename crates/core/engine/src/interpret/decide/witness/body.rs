//! [`Body`]: a page's summary from its own text, its first paragraph cut near a sentence end.

use model::code::Summary;
use model::{Block, Evidence, Inline, Visit};

use crate::interpret::decide::Sourced;
use crate::interpret::decide::ladder::{Case, Testimony, Witness};
use crate::interpret::decide::trace::{Named, Witnessed};

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

/// Where reading a paragraph as plain text stopped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Stop {
    /// It read every inline.
    #[default]
    End,
    /// At math: TeX is not text a reader reads, and a summary has no `MathML`, so the text
    /// stops before it.
    Math,
}

/// Inlines as plain text: what a reader would read, breaks as spaces, images left out, up to
/// the first math.
#[derive(Debug, Default)]
struct Plain {
    text: String,
    stop: Stop,
}

impl Visit for Plain {
    fn inline(&mut self, inline: &Inline) {
        if self.stop == Stop::Math {
            return;
        }
        match inline {
            Inline::Code(text) | Inline::Keyboard(text) => self.text.push_str(text),
            Inline::Break | Inline::SoftBreak => self.text.push(' '),
            Inline::Math(_) => self.stop = Stop::Math,
            Inline::Image { .. } => {}
            other => other.walk(self),
        }
    }

    fn text(&mut self, text: &str) {
        if self.stop == Stop::End {
            self.text.push_str(text);
        }
    }
}

impl Plain {
    /// The text, its runs of space made one, and when it stopped at math, only its sentences
    /// before the math (or what came before, then `…`); then whole when short; else up to the
    /// sentence end nearest [`NEAR`] between [`SHORTEST`] and [`LONGEST`] characters; else up to
    /// the last space before [`NEAR`], then `…`.
    fn cut(self) -> String {
        let mut text = self.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if self.stop == Stop::Math {
            let mut ends = text.char_indices().rev().filter(|&(at, c)| {
                matches!(c, '.' | '!' | '?')
                    && text.get(at + c.len_utf8()..).is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
            });
            match ends.next() {
                Some((at, c)) => text.truncate(at + c.len_utf8()),
                None if text.is_empty() => {}
                None => text.push('…'),
            }
        }
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

impl Named for Body {
    const NAME: Witnessed = Witnessed::Body;
}

#[cfg(test)]
mod test {
    use super::*;

    fn cut(text: &str) -> String {
        Plain { text: text.to_owned(), stop: Stop::End }.cut()
    }

    fn plain(inlines: &[Inline]) -> String {
        let mut plain = Plain::default();
        plain.inlines(inlines);
        plain.cut()
    }

    #[test]
    fn a_summary_stops_before_math() {
        let text = |text: &str| Inline::Text(text.to_owned());
        let math = Inline::Math("x^2".to_owned());
        assert_eq!(plain(&[text("Ink flows. Let "), math.clone(), text(" grow.")]), "Ink flows.");
        assert_eq!(plain(&[text("Let "), math.clone(), text(" grow.")]), "Let…");
        assert_eq!(plain(&[math, text(" grows.")]), "");
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
