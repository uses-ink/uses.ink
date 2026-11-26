//! `page.summary`: one or two sentences for feeds, listings and `og:description`.

use std::fmt;

use model::{Block, DefaultReason, Evidence, Inline};

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// The length a summary is cut near, in characters.
const NEAR: usize = 160;
/// A sentence end is taken from this many characters on.
const SHORTEST: usize = 80;
/// And up to this many.
const LONGEST: usize = 240;

/// Pin, then frontmatter `description` or `summary`, then the first paragraph cut at a sentence
/// end near [`NEAR`] characters, then none for a page with no paragraph.
pub(crate) fn decide(source: &Source, pins: &impl Pins, record: &mut Record) {
    let front = &source.read.front;
    let first = source.read.document.blocks.iter().find_map(|block| match block {
        Block::Paragraph(inlines) => Some(PlainText(inlines).to_string()),
        _ => None,
    });
    let body = first.map(|text| cut(&text)).filter(|text| !text.is_empty());
    Ladder::<code::Summary>::new(source.subject(), pins, record)
        .rung(front.description.as_ref().map(|text| (Some(text.value.clone()), source.frontmatter(text.span))))
        .rung(body.map(|text| (Some(text), Evidence::Body { path: source.path.clone() })))
        .or_default(None, DefaultReason::NothingMatched, record);
}

/// `text` whole when short; else up to the sentence end nearest [`NEAR`] between [`SHORTEST`]
/// and [`LONGEST`] characters; else up to the last space before [`NEAR`], then `…`.
fn cut(text: &str) -> String {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.len() <= NEAR {
        return text.to_owned();
    }
    let ends = chars.iter().enumerate().filter_map(|(at, &(byte, c))| {
        let next = chars.get(at + 1).map(|&(_, next)| next);
        (matches!(c, '.' | '!' | '?') && next.is_none_or(char::is_whitespace)).then_some((at, byte + c.len_utf8()))
    });
    let nearest = ends.filter(|(at, _)| (SHORTEST..LONGEST).contains(at)).min_by_key(|(at, _)| at.abs_diff(NEAR));
    if let Some((_, byte)) = nearest {
        return text.get(..byte).unwrap_or(text).to_owned();
    }
    let space = chars.iter().take(NEAR).rev().find(|(_, c)| c.is_whitespace()).map(|&(byte, _)| byte);
    let byte = space.or_else(|| chars.get(NEAR).map(|&(byte, _)| byte)).unwrap_or(text.len());
    format!("{}…", text.get(..byte).unwrap_or(text).trim_end())
}

/// Inlines as plain text: what a reader would read, breaks as spaces, images left out.
#[derive(Debug)]
struct PlainText<'a>(&'a [Inline]);

impl fmt::Display for PlainText<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for inline in self.0 {
            match inline {
                Inline::Text(text) | Inline::Code(text) | Inline::Math(text) | Inline::Keyboard(text) => {
                    out.write_str(text)?;
                }
                Inline::Emphasis(children)
                | Inline::Strong(children)
                | Inline::Strike(children)
                | Inline::Highlight(children)
                | Inline::Superscript(children)
                | Inline::Subscript(children)
                | Inline::Link { children, .. } => write!(out, "{}", PlainText(children))?,
                Inline::Break | Inline::SoftBreak => out.write_str(" ")?,
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
