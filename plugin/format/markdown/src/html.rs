//! Raw HTML, taken apart far enough to keep what decision 0027 allows and remove the rest: tags
//! with their attributes, comments and the text between. Nothing here builds a tree; the body
//! pairs tags.

mod attribute;
mod element;

pub(crate) use attribute::Attributes;
pub(crate) use element::Element;

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
    /// The tag's attributes, in the order written.
    pub(crate) fn attributes(&self) -> Attributes<'a> {
        Attributes::new(self.rest)
    }

    /// The value of the first attribute called `name` (lower case), entities decoded; an
    /// attribute written without a value has the empty text.
    pub(crate) fn attribute(&self, name: &str) -> Option<String> {
        self.attributes().find(|attribute| attribute.name == name).map(|attribute| attribute.value)
    }
}

/// Elements whose content is code, never prose.
const OPAQUE: &[&str] = &["script", "style"];

impl Piece<'_> {
    /// Whether this tag opens an element whose content is not text to keep.
    pub(crate) fn opens_opaque(&self) -> bool {
        matches!(self, Self::Open(tag) if OPAQUE.contains(&tag.name.as_str()))
    }

    /// Whether this tag closes an element [`Self::opens_opaque`] opened.
    pub(crate) fn closes_opaque(&self) -> bool {
        matches!(self, Self::Close(tag) if OPAQUE.contains(&tag.name.as_str()))
    }
}

/// The pieces of `html`, in order.
#[derive(Debug)]
pub(crate) struct Pieces<'a> {
    html: &'a str,
    at: usize,
}

impl<'a> Pieces<'a> {
    /// The pieces of `html`, from its start.
    pub(crate) fn new(html: &'a str) -> Self {
        Self { html, at: 0 }
    }

    /// A tag at `start`, which holds `<`: its end, after the closing `>` (or the end of the text).
    fn tag_end(&self, start: usize) -> usize {
        let rest = self.html.get(start..).unwrap_or_default();
        if rest.starts_with("<!--") {
            return rest
                .get(4..)
                .and_then(|after| after.find("-->"))
                .map_or(self.html.len(), |end| start + 4 + end + 3);
        }
        let mut quote = None;
        for (offset, c) in rest.char_indices().skip(1) {
            match (quote, c) {
                (None, '"' | '\'') => quote = Some(c),
                (Some(open), _) if open == c => quote = None,
                (None, '>') => return start + offset + 1,
                _ => {}
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
        let tag = rest.match_indices('<').map(|(open, _)| open).find(|&open| rest.get(open..).is_some_and(is_tag));
        let Some(open) = tag else {
            self.at = self.html.len();
            return Some(Piece::Text(rest));
        };
        if open > 0 {
            self.at = start + open;
            return rest.get(..open).map(Piece::Text);
        }
        let end = self.tag_end(start);
        self.at = end;
        let tag = self.html.get(start..end).unwrap_or_default();
        Some(piece(tag, start, end))
    }
}

/// Whether text starting with `<` starts a tag rather than a lone `<`.
fn is_tag(text: &str) -> bool {
    let mut chars = text.chars().skip(1);
    match chars.next() {
        Some('/') => chars.next().is_some_and(|c| c.is_ascii_alphabetic()),
        Some(c) => c.is_ascii_alphabetic() || c == '!' || c == '?',
        None => false,
    }
}

fn piece(tag: &str, start: usize, end: usize) -> Piece<'_> {
    if tag.starts_with("<!--") {
        return Piece::Comment;
    }
    if let Some(after) = tag.strip_prefix("</") {
        let (name, rest) = name(after);
        return Piece::Close(Tag { name, start, end, rest });
    }
    let (name, rest) = name(tag.get(1..).unwrap_or_default());
    Piece::Open(Tag { name, start, end, rest })
}

/// The tag name at the start of `text`, lower-cased, and the text after it.
fn name(text: &str) -> (String, &str) {
    let end = text.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(text.len());
    let (name, rest) = match text.get(..end).filter(|name| !name.is_empty()) {
        Some(name) => (name, text.get(end..).unwrap_or_default()),
        None => (text, ""),
    };
    (name.chars().take(64).collect::<String>().to_ascii_lowercase(), rest)
}

/// Text from HTML with the common character references decoded: the five XML ones, `&nbsp;`,
/// and numeric ones. Any other `&` stays as written.
pub(crate) fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(rest.get(..amp).unwrap_or_default());
        let after = rest.get(amp..).unwrap_or_default();
        let (c, len) = reference(after).unwrap_or(('&', 1));
        out.push(c);
        rest = after.get(len..).unwrap_or_default();
    }
    out.push_str(rest);
    out
}

/// The character a reference at the start of `text` stands for, and the reference's length.
fn reference(text: &str) -> Option<(char, usize)> {
    let end = text.get(..12).unwrap_or(text).find(';')?;
    let body = text.get(1..end)?;
    let c = match body {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        _ => {
            let number = body.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code).filter(|&c| c != '\0')?
        }
    };
    Some((c, end + 1))
}

#[cfg(test)]
mod test {
    use super::{Piece, Pieces, Tag, decode};

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
    fn reads_attributes_of_a_tag() {
        let Some(Piece::Open(tag)) = Pieces::new("<img SRC='a.png' alt=\"A &amp; B\" hidden>").next() else {
            panic!("not a tag");
        };
        assert_eq!(tag.attribute("src").as_deref(), Some("a.png"));
        assert_eq!(tag.attribute("alt").as_deref(), Some("A & B"));
        assert_eq!(tag.attribute("hidden").as_deref(), Some(""));
        assert_eq!(tag.attribute("title"), None);
    }

    #[test]
    fn decodes_common_references() {
        assert_eq!(decode("a &amp; b &lt;&#x41;&#66;&bogus; &"), "a & b <AB&bogus; &");
    }
}
