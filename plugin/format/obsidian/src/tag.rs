//! Tags in the text (`#tag`, `#a/b`), gathered while the body is read and joined to the
//! frontmatter's.

use model::{Front, Span, Spanned, Tag};

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

    /// Adds the tags to `front`'s, after the frontmatter's and without one whose slug is already
    /// there. With no tags in the frontmatter, the first tag in the text is where they are
    /// written.
    pub(crate) fn join(self, front: &mut Front) {
        let Some(first) = self.0.first().map(|tag| tag.span) else { return };
        let tags = front.tags.get_or_insert_with(|| Spanned::new(Vec::new(), first));
        for Spanned { value, .. } in self.0 {
            if !tags.value.iter().any(|tag| tag.slug == value.slug) {
                tags.value.push(value);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use model::{Front, Span};

    use super::Tags;

    fn tags(text: &str) -> Vec<String> {
        let mut tags = Tags::default();
        tags.scan(text, Span::new(10, 10 + text.len()).unwrap());
        let mut front = Front::default();
        tags.join(&mut front);
        front.tags.map_or_else(Vec::new, |tags| tags.value.into_iter().map(|tag| tag.written).collect())
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
