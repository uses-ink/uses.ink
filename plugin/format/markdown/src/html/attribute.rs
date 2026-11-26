//! A tag's attributes, read the way a browser reads them: `name`, `name=value`, `name='value'`
//! and `name="value"`, separated by whitespace.

use super::decode;

/// One attribute: its name lower-cased and its value with entities decoded. An attribute written
/// without a value (`hidden`) has the empty text.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Attribute {
    pub(crate) name: String,
    pub(crate) value: String,
}

/// The attributes in the text of a tag after its name, up to its closing `>` or `/>`.
#[derive(Debug)]
pub(crate) struct Attributes<'a> {
    rest: &'a str,
}

impl<'a> Attributes<'a> {
    /// The attributes written in `rest`, the text after a tag's name.
    pub(crate) fn new(rest: &'a str) -> Self {
        Self { rest }
    }

    /// Takes from the front of what is left while `keep` holds; returns what it took.
    fn take_while(&mut self, keep: impl Fn(char) -> bool) -> &'a str {
        let end = self.rest.find(|c: char| !keep(c)).unwrap_or(self.rest.len());
        let (taken, rest) = self.rest.split_at_checked(end).unwrap_or((self.rest, ""));
        self.rest = rest;
        taken
    }

    /// Takes `c` from the front, if it is there.
    fn eat(&mut self, c: char) -> bool {
        match self.rest.strip_prefix(c) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    /// The value after `=`: quoted, or up to the next whitespace or `>`.
    fn value(&mut self) -> &'a str {
        self.take_while(char::is_whitespace);
        for quote in ['"', '\''] {
            if self.eat(quote) {
                let value = self.take_while(|c| c != quote);
                self.eat(quote);
                return value;
            }
        }
        self.take_while(|c| !c.is_whitespace() && c != '>')
    }
}

impl Iterator for Attributes<'_> {
    type Item = Attribute;

    fn next(&mut self) -> Option<Attribute> {
        loop {
            self.take_while(|c| c.is_whitespace() || c == '/');
            if self.rest.is_empty() || self.rest.starts_with('>') {
                return None;
            }
            let name = self.take_while(|c| !c.is_whitespace() && !matches!(c, '=' | '>' | '/'));
            if name.is_empty() {
                // a stray `=`: a browser skips it, and so does this
                self.eat('=');
                continue;
            }
            self.take_while(char::is_whitespace);
            let value = if self.eat('=') { decode(self.value()) } else { String::new() };
            return Some(Attribute { name: name.to_ascii_lowercase(), value });
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Attribute, Attributes};

    fn read(rest: &str) -> Vec<(String, String)> {
        Attributes::new(rest).map(|Attribute { name, value }| (name, value)).collect()
    }

    #[test]
    fn reads_every_form() {
        let found = read(r#" a=1 B = "two words" c='it''s' d e=x>y f="#);
        let expected = [("a", "1"), ("b", "two words"), ("c", "it"), ("'s'", ""), ("d", ""), ("e", "x")];
        let expected: Vec<_> = expected.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())).collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn stops_at_the_end_of_the_tag() {
        assert_eq!(read(" src=a.png/>"), [("src".to_owned(), "a.png/".to_owned())]);
        assert_eq!(read("/>"), []);
        assert_eq!(read(" = k>"), [("k".to_owned(), String::new())]);
    }
}
