//! Tags in the text (`#tag`, `#a/b`), gathered while the body is read and given as the read's
//! `inline_tags`, apart from the frontmatter's.

use model::{Read, Span, Spanned, Tag};

/// The tags of one note's text, in the order written, each with where it is.
#[derive(Debug, Default)]
pub(crate) struct Tags(Vec<Spanned<Tag>>);

impl Tags {
    /// Gathers the tags of `text`, written at `span`. A tag is `#` at the start of the text or
    /// after whitespace, then letters, digits, `_`, `-` and `/`, holding something other than
    /// digits (`#1` is no tag); a `/` it ends with is not part of it. When the span is the text's
    /// own (as long as it), each tag is placed exactly; otherwise at the whole span.
    pub(crate) fn scan(&mut self, text: &str, span: Span) {
        let exact = span.len() == text.len();
        let mut before = None;
        let mut chars = text.char_indices();
        while let Some((at, c)) = chars.next() {
            let starts = c == '#' && before.is_none_or(char::is_whitespace);
            before = Some(c);
            if !starts {
                continue;
            }
            let rest = text.get(at + 1..).unwrap_or_default();
            let length =
                rest.find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))).unwrap_or(rest.len());
            let name = rest.get(..length).unwrap_or_default().trim_end_matches('/');
            if name.is_empty() || name.bytes().all(|byte| byte.is_ascii_digit()) {
                continue;
            }
            let Ok(tag) = Tag::new(name) else { continue };
            let place =
                if exact { Span::new(span.start() + at, span.start() + at + 1 + name.len()).ok() } else { None };
            self.0.push(Spanned::new(tag, place.unwrap_or(span)));
            // the tag's own characters start no tag
            for _ in name.chars() {
                before = chars.next().map(|(_, c)| c);
            }
        }
    }

    /// Gives the tags, in the order written and each slug once, as `read`'s inline tags. The
    /// frontmatter's are not touched.
    pub(crate) fn into(self, read: &mut Read) {
        for tag in self.0 {
            if !read.inline_tags.iter().any(|held| held.value.slug == tag.value.slug) {
                read.inline_tags.push(tag);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use model::{Document, Front, Read, Span};

    use super::Tags;

    fn tags(text: &str) -> Vec<String> {
        let mut tags = Tags::default();
        tags.scan(text, Span::new(10, 10 + text.len()).unwrap());
        let mut read = Read::new(Document::default(), Front::default(), None);
        tags.into(&mut read);
        assert!(read.front.tags.is_none());
        read.inline_tags.into_iter().map(|tag| tag.value.written).collect()
    }

    #[test]
    fn tags_start_after_whitespace_and_hold_more_than_digits() {
        assert_eq!(tags("#one two#no #2024 #a/b/ #x-y_z, #One"), ["one", "a/b", "x-y_z"]);
    }

    #[test]
    fn a_tag_in_the_text_is_placed_exactly() {
        let mut tags = Tags::default();
        tags.scan("see #rust", Span::new(4, 13).unwrap());
        assert_eq!(tags.0[0].span, Span::new(8, 13).unwrap());
    }
}
