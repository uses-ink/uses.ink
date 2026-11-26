//! Raw HTML, taken apart only far enough to remove it: tags, comments and the text between.

/// One piece of raw HTML, with its byte range in the HTML it came from.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Piece<'a> {
    /// Text between tags, as written.
    Text(&'a str),
    /// `<name ...>` or `<name/>`, the name lower-cased; also `<!doctype>` and `<?...?>`, named
    /// by what follows the `<`.
    Open { name: String, start: usize, end: usize },
    /// `</name>`, the name lower-cased.
    Close { name: String },
    /// `<!-- ... -->`.
    Comment,
}

/// Elements whose content is code, never prose.
const OPAQUE: &[&str] = &["script", "style"];

impl Piece<'_> {
    /// Whether this tag opens an element whose content is not text to keep.
    pub(crate) fn opens_opaque(&self) -> bool {
        matches!(self, Self::Open { name, .. } if OPAQUE.contains(&name.as_str()))
    }

    pub(crate) fn closes_opaque(&self) -> bool {
        matches!(self, Self::Close { name } if OPAQUE.contains(&name.as_str()))
    }
}

/// The pieces of `html`, in order.
#[derive(Debug)]
pub(crate) struct Pieces<'a> {
    html: &'a str,
    at: usize,
}

impl<'a> Pieces<'a> {
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
    if let Some(rest) = tag.strip_prefix("</") {
        return Piece::Close { name: name(rest) };
    }
    Piece::Open { name: name(tag.get(1..).unwrap_or_default()), start, end }
}

/// The tag name at the start of `text`, lower-cased.
fn name(text: &str) -> String {
    let end = text.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(text.len());
    let name = text.get(..end).filter(|name| !name.is_empty()).unwrap_or(text);
    name.chars().take(64).collect::<String>().to_ascii_lowercase()
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
    use super::{Piece, Pieces, decode};

    #[test]
    fn splits_tags_from_text() {
        let pieces: Vec<_> = Pieces::new("a <b class=\"x>y\">c</b> <!-- d --> 1 < 2 <i>").collect();
        assert_eq!(
            pieces,
            [
                Piece::Text("a "),
                Piece::Open { name: "b".into(), start: 2, end: 17 },
                Piece::Text("c"),
                Piece::Close { name: "b".into() },
                Piece::Text(" "),
                Piece::Comment,
                Piece::Text(" 1 < 2 "),
                Piece::Open { name: "i".into(), start: 40, end: 43 },
            ]
        );
    }

    #[test]
    fn decodes_common_references() {
        assert_eq!(decode("a &amp; b &lt;&#x41;&#66;&bogus; &"), "a & b <AB&bogus; &");
    }
}
