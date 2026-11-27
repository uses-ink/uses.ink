//! Raw HTML, taken apart far enough to keep what decision 0027 allows and remove the rest: tags
//! with their attributes, comments and the text between. Nothing here builds a tree or knows the
//! allowlist; the body pairs tags and decides what each becomes.

mod attribute;
mod text;

pub(crate) use attribute::Attributes;
pub(crate) use text::{Collapsed, Decoded};

/// Elements whose content is code, never prose.
const OPAQUE: &[&str] = &["script", "style"];

/// The longest tag name kept: a longer one is cut, so a hostile name costs nothing.
const NAME: usize = 64;

/// One piece of raw HTML.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Piece<'a> {
    /// Text between tags, as written.
    Text(&'a str),
    /// `<name ...>` or `<name/>`; also `<!doctype>` and `<?...?>`, named by what follows the `<`.
    Open(Tag<'a>),
    /// `</name>`.
    Close(Tag<'a>),
    /// `<!-- ... -->`.
    Comment,
}

impl<'a> Piece<'a> {
    /// The piece a whole tag is, `html` being its text from `<` to `>`, at `start..end` in the
    /// HTML it came from.
    fn of(html: &'a str, start: usize, end: usize) -> Self {
        if html.starts_with("<!--") {
            return Self::Comment;
        }
        match html.strip_prefix("</") {
            Some(after) => Self::Close(Tag::of(after, start, end)),
            None => Self::Open(Tag::of(html.get(1..).unwrap_or_default(), start, end)),
        }
    }

    /// The tag, for an opening or a closing tag.
    pub(crate) const fn tag(&self) -> Option<&Tag<'a>> {
        match self {
            Self::Open(tag) | Self::Close(tag) => Some(tag),
            Self::Text(_) | Self::Comment => None,
        }
    }

    /// Whether this tag opens an element whose content is not text to keep.
    pub(crate) fn opens_opaque(&self) -> bool {
        matches!(self, Self::Open(tag) if tag.is_opaque())
    }

    /// Whether this tag closes an element [`Self::opens_opaque`] opened.
    pub(crate) fn closes_opaque(&self) -> bool {
        matches!(self, Self::Close(tag) if tag.is_opaque())
    }
}

/// One tag: its name lower-cased, its byte range in the HTML it came from, and the text after
/// its name, where its attributes are.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Tag<'a> {
    pub(crate) name: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    rest: &'a str,
}

impl<'a> Tag<'a> {
    /// The tag whose text after `<` or `</` is `text`: its name up to a space, `>` or `/`.
    fn of(text: &'a str, start: usize, end: usize) -> Self {
        let cut = text.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(text.len());
        let (name, rest) = match text.get(..cut).filter(|name| !name.is_empty()) {
            Some(name) => (name, text.get(cut..).unwrap_or_default()),
            None => (text, ""),
        };
        let name = name.chars().take(NAME).collect::<String>().to_ascii_lowercase();
        Self { name, start, end, rest }
    }

    /// The tag's attributes, in the order written.
    pub(crate) fn attributes(&self) -> Attributes<'a> {
        Attributes::new(self.rest)
    }

    /// The value of the first attribute called `name` (lower case), entities decoded; an
    /// attribute written without a value has the empty text.
    pub(crate) fn attribute(&self, name: &str) -> Option<String> {
        self.attributes().find(|attribute| attribute.name == name).map(|attribute| attribute.value)
    }

    fn is_opaque(&self) -> bool {
        OPAQUE.contains(&self.name.as_str())
    }
}

/// The pieces of some HTML, in order.
#[derive(Debug)]
pub(crate) struct Pieces<'a> {
    html: &'a str,
    at: usize,
}

impl<'a> Pieces<'a> {
    /// The pieces of `html`, from its start.
    pub(crate) const fn new(html: &'a str) -> Self {
        Self { html, at: 0 }
    }

    /// Whether text starting with `<` starts a tag rather than a lone `<`.
    fn starts(text: &str) -> bool {
        let mut chars = text.chars().skip(1);
        match chars.next() {
            Some('/') => chars.next().is_some_and(|c| c.is_ascii_alphabetic()),
            Some(c) => c.is_ascii_alphabetic() || c == '!' || c == '?',
            None => false,
        }
    }

    /// The end of a tag starting at `start`, which holds `<`: after its closing `>` outside
    /// quotes, after `-->` for a comment, or the end of the text. A quote opens a value only
    /// right after `=`, as in a browser: `alt=Ben's` holds an apostrophe, not a quote.
    fn end(&self, start: usize) -> usize {
        let rest = self.html.get(start..).unwrap_or_default();
        if rest.starts_with("<!--") {
            return rest
                .get(4..)
                .and_then(|after| after.find("-->"))
                .map_or(self.html.len(), |end| start + 4 + end + 3);
        }
        let mut quote = None;
        let mut before = ' ';
        for (offset, c) in rest.char_indices().skip(1) {
            match (quote, c) {
                (None, '"' | '\'') if before == '=' => quote = Some(c),
                (Some(open), _) if open == c => quote = None,
                (None, '>') => return start + offset + 1,
                _ => {}
            }
            if quote.is_none() && !c.is_whitespace() {
                before = c;
            }
        }
        self.html.len()
    }
}

impl<'a> Iterator for Pieces<'a> {
    type Item = Piece<'a>;

    fn next(&mut self) -> Option<Piece<'a>> {
        let rest = self.html.get(self.at..).filter(|rest| !rest.is_empty())?;
        let start = self.at;
        let tag =
            rest.match_indices('<').map(|(open, _)| open).find(|&open| rest.get(open..).is_some_and(Self::starts));
        let Some(open) = tag else {
            self.at = self.html.len();
            return Some(Piece::Text(rest));
        };
        if open > 0 {
            self.at = start + open;
            return rest.get(..open).map(Piece::Text);
        }
        let end = self.end(start);
        self.at = end;
        Some(Piece::of(self.html.get(start..end).unwrap_or_default(), start, end))
    }
}

#[cfg(test)]
mod test {
    use super::{Piece, Pieces, Tag};

    #[test]
    fn splits_tags_from_text() {
        let pieces: Vec<_> = Pieces::new("a <B class=\"x>y\">c</b> <!-- d --> 1 < 2 <i>").collect();
        assert_eq!(
            pieces,
            [
                Piece::Text("a "),
                Piece::Open(Tag { name: "b".into(), start: 2, end: 17, rest: " class=\"x>y\">" }),
                Piece::Text("c"),
                Piece::Close(Tag { name: "b".into(), start: 18, end: 22, rest: ">" }),
                Piece::Text(" "),
                Piece::Comment,
                Piece::Text(" 1 < 2 "),
                Piece::Open(Tag { name: "i".into(), start: 40, end: 43, rest: ">" }),
            ]
        );
    }

    #[test]
    fn an_apostrophe_in_an_unquoted_value_opens_no_quote() {
        let pieces: Vec<_> = Pieces::new("<img alt=Ben's src=a.png>\ntext\n<img alt=x src=y>").collect();
        let kinds: Vec<&str> = pieces
            .iter()
            .map(|piece| match piece {
                Piece::Open(_) => "open",
                Piece::Text(_) => "text",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["open", "text", "open"], "{pieces:?}");
    }

    #[test]
    fn reads_attributes_of_a_tag() {
        let Some(Piece::Open(tag)) = Pieces::new("<img SRC='a.png' alt=\"A &amp; B\" hidden>").next() else {
            panic!("not a tag");
        };
        assert_eq!(tag.attribute("src").as_deref(), Some("a.png"));
        assert_eq!(tag.attribute("alt").as_deref(), Some("A & B"));
        assert_eq!(tag.attribute("hidden").as_deref(), Some(""));
        assert_eq!(tag.attribute("title"), None);
    }
}
