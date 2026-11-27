//! Entity tags from content hashes, and the conditions that compare them.

use std::fmt;

use model::Hash;

use crate::condition::ConditionError;
use crate::encoding::Encoding;

/// Whether a tag promises the same bytes or only the same meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    /// `"x"`.
    Strong,
    /// `W/"x"`.
    Weak,
}

/// An entity tag: the hash of the bytes served, with the coding when they are a variant. The
/// same blob always has the same tag, on every deploy that holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    strength: Strength,
    opaque: String,
}

impl Tag {
    /// The strong tag of `hash`, as a `-br` or `-gzip` variant when `encoding` is given.
    pub fn of(hash: Hash, encoding: Option<Encoding>) -> Self {
        let opaque = match encoding {
            Some(encoding) => format!("{hash}-{encoding}"),
            None => hash.to_string(),
        };
        Self { strength: Strength::Strong, opaque }
    }

    /// Whether the two name the same bytes or meaning: weak comparison, the one `If-None-Match` uses.
    pub fn matches(&self, other: &Self) -> bool {
        self.opaque == other.opaque
    }

    /// Whether the two name the same bytes: both strong and equal, the comparison `If-Range` uses.
    pub fn is_same(&self, other: &Self) -> bool {
        self.strength == Strength::Strong && other.strength == Strength::Strong && self.opaque == other.opaque
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.strength {
            Strength::Strong => write!(out, "\"{}\"", self.opaque),
            Strength::Weak => write!(out, "W/\"{}\"", self.opaque),
        }
    }
}

/// A position in a header's value.
#[derive(Debug)]
pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn at(&self) -> usize {
        self.at
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    pub(crate) fn step(&mut self) {
        self.at += 1;
    }

    /// Skips spaces and tabs.
    pub(crate) fn skip(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.step();
        }
    }

    /// Whether only spaces and tabs are left.
    pub(crate) fn is_done(&mut self) -> bool {
        self.skip();
        self.peek().is_none()
    }

    /// One tag, `"x"` or `W/"x"`; the offset it went wrong at otherwise.
    pub(crate) fn tag(&mut self) -> Result<Tag, usize> {
        let mut strength = Strength::Strong;
        if self.bytes.get(self.at..self.at + 2) == Some(b"W/") {
            strength = Strength::Weak;
            self.at += 2;
        }
        if self.peek() != Some(b'"') {
            return Err(self.at);
        }
        self.step();
        let start = self.at;
        while let Some(byte) = self.peek() {
            match byte {
                b'"' => {
                    let inner = self.bytes.get(start..self.at).unwrap_or_default();
                    self.step();
                    let opaque = std::str::from_utf8(inner).map_err(|_| start)?.to_owned();
                    return Ok(Tag { strength, opaque });
                }
                0x21 | 0x23..=0x7e => self.step(),
                _ => return Err(self.at),
            }
        }
        Err(self.at)
    }
}

/// An `If-None-Match`: any tag, or a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Matching {
    /// `*`.
    Any,
    /// Each tag listed.
    Tags(Vec<Tag>),
}

impl Matching {
    /// Reads the header's value; at most `limit` tags.
    pub(crate) fn read(value: &[u8], limit: usize) -> Result<Self, ConditionError> {
        let mut cursor = Cursor::new(value);
        cursor.skip();
        if cursor.peek() == Some(b'*') {
            cursor.step();
            return if cursor.is_done() { Ok(Self::Any) } else { Err(ConditionError::IfNoneMatch { at: cursor.at() }) };
        }
        let mut tags = Vec::new();
        loop {
            cursor.skip();
            match cursor.peek() {
                None => break,
                Some(b',') => cursor.step(),
                Some(_) => {
                    tags.push(cursor.tag().map_err(|at| ConditionError::IfNoneMatch { at })?);
                    if tags.len() > limit {
                        return Err(ConditionError::Tags { count: tags.len(), limit });
                    }
                    cursor.skip();
                    match cursor.peek() {
                        None | Some(b',') => {}
                        Some(_) => return Err(ConditionError::IfNoneMatch { at: cursor.at() }),
                    }
                }
            }
        }
        Ok(Self::Tags(tags))
    }

    /// Whether `tag` is one the client already holds.
    pub fn holds(&self, tag: &Tag) -> bool {
        match self {
            Self::Any => true,
            Self::Tags(tags) => tags.iter().any(|held| held.matches(tag)),
        }
    }
}

/// An `If-Range`: the tag the client's partial copy has, or a date this edge cannot compare
/// (it keeps no modification times), which never holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Validator {
    /// An entity tag.
    Tag(Tag),
    /// An HTTP date.
    Date,
}

impl Validator {
    pub(crate) fn read(value: &[u8]) -> Result<Self, ConditionError> {
        let mut cursor = Cursor::new(value);
        cursor.skip();
        let trimmed = value.trim_ascii_start();
        if trimmed.starts_with(b"\"") || trimmed.starts_with(b"W/") {
            let tag = cursor.tag().map_err(|at| ConditionError::IfRange { at })?;
            return if cursor.is_done() {
                Ok(Self::Tag(tag))
            } else {
                Err(ConditionError::IfRange { at: cursor.at() })
            };
        }
        if value.is_empty() || !value.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
            return Err(ConditionError::IfRange { at: 0 });
        }
        Ok(Self::Date)
    }

    /// Whether the client's copy is the one `current` names.
    pub fn holds(&self, current: &Tag) -> bool {
        match self {
            Self::Tag(tag) => tag.is_same(current),
            Self::Date => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn writes_the_hash_as_a_strong_tag() {
        let hash = Hash::of(b"x");
        assert_eq!(Tag::of(hash, None).to_string(), format!("\"{hash}\""));
        assert_eq!(Tag::of(hash, Some(Encoding::Brotli)).to_string(), format!("\"{hash}-br\""));
    }

    #[test]
    fn matches_weakly_and_lists() {
        let tag = Tag::of(Hash::of(b"x"), None);
        let held = |text: &str| Matching::read(text.as_bytes(), 16).unwrap().holds(&tag);
        assert!(held(&format!("{tag}")));
        assert!(held(&format!("W/{tag}")));
        assert!(held(&format!("\"other\" , {tag}")));
        assert!(held("*"));
        assert!(!held("\"other\""));
        assert!(!held(""));
    }

    #[test]
    fn refuses_a_malformed_list() {
        for bad in ["x", "\"a", "\"a\" \"b\"", "* , \"a\"", "W/x", "\"a\u{7f}\""] {
            assert!(matches!(Matching::read(bad.as_bytes(), 16), Err(ConditionError::IfNoneMatch { .. })), "{bad}");
        }
        assert_eq!(Matching::read(b"\"a\",\"b\",\"c\"", 2), Err(ConditionError::Tags { count: 3, limit: 2 }));
    }

    #[test]
    fn an_if_range_holds_for_a_strong_tag_only() {
        let tag = Tag::of(Hash::of(b"x"), None);
        assert!(Validator::read(tag.to_string().as_bytes()).unwrap().holds(&tag));
        assert!(!Validator::read(format!("W/{tag}").as_bytes()).unwrap().holds(&tag));
        assert!(!Validator::read(b"Wed, 21 Oct 2015 07:28:00 GMT").unwrap().holds(&tag));
    }
}
