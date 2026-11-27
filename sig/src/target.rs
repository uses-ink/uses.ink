//! The request target, in canonical form or not at all.

use std::fmt;

use crate::alphabet::Crockford;

/// Why a target is not in canonical form. The verifier does not repair a target: it refuses one
/// that is not canonical, before a signature is looked at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotCanonical {
    /// Nothing at all.
    Empty,
    /// Does not start with `/`.
    Start,
    /// A byte that is neither unreserved, `/`, nor a `%XX` escape, at this offset. A fragment,
    /// a second `?` and a space land here.
    Character {
        /// The offset.
        at: usize,
    },
    /// A `%` that is not followed by two upper-case hexadecimal digits.
    Escape {
        /// The offset of the `%`.
        at: usize,
    },
    /// An escape of a byte that is never escaped (`%7E` for `~`).
    Unreserved {
        /// The offset of the `%`.
        at: usize,
    },
    /// An escaped `/`, which would hide a segment boundary.
    Delimiter {
        /// The offset of the `%`.
        at: usize,
    },
    /// A segment that is `.` or `..`.
    Dot {
        /// The offset of the segment.
        at: usize,
    },
    /// An empty path segment (`//`, or a trailing `/`).
    Segment {
        /// The offset where it should have text.
        at: usize,
    },
    /// An empty query pair (`?`, `&&`, a trailing `&`).
    Pair {
        /// The offset where it should have text.
        at: usize,
    },
    /// A query pair with no `=`.
    Equals {
        /// The offset of the pair.
        at: usize,
    },
    /// A query pair with an empty name.
    Name {
        /// The offset of the pair.
        at: usize,
    },
    /// Query pairs not sorted by name and then value, as bytes.
    Order {
        /// The offset of the pair that is out of place.
        at: usize,
    },
}

impl fmt::Display for NotCanonical {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("the target is empty"),
            Self::Start => out.write_str("the target does not start with `/`"),
            Self::Character { at } => write!(out, "byte {at} of the target is not in the canonical alphabet"),
            Self::Escape { at } => write!(out, "the `%` at byte {at} is not followed by two upper-case hex digits"),
            Self::Unreserved { at } => write!(out, "the escape at byte {at} is of a byte that is never escaped"),
            Self::Delimiter { at } => write!(out, "the escape at byte {at} hides a `/`"),
            Self::Dot { at } => write!(out, "the segment at byte {at} is `.` or `..`"),
            Self::Segment { at } => write!(out, "the path has an empty segment at byte {at}"),
            Self::Pair { at } => write!(out, "the query has an empty pair at byte {at}"),
            Self::Equals { at } => write!(out, "the query pair at byte {at} has no `=`"),
            Self::Name { at } => write!(out, "the query pair at byte {at} has no name"),
            Self::Order { at } => write!(out, "the query pair at byte {at} is out of order"),
        }
    }
}

impl std::error::Error for NotCanonical {}

/// A request target that is in canonical form (`docs/auth.md` section 5): the path and, if
/// present, `?` and the query, as sent.
///
/// - The path is `/`-separated segments, none empty, `.` or `..`.
/// - A byte outside the unreserved set (`A-Z a-z 0-9 - . _ ~`) and `/` is written `%XX` with
///   upper-case hex; an unreserved byte is never escaped, and `/` is never escaped.
/// - The query is `name=value` pairs joined by `&`, sorted by name and then value as bytes, no
///   empty pair, no fragment.
///
/// The alphabet has no line break, so a target cannot shift a line of the signed text.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Target(String);

impl Target {
    /// `text` as a target, or why it is not canonical.
    pub fn parse(text: &str) -> Result<Self, NotCanonical> {
        let bytes = text.as_bytes();
        match bytes.first() {
            None => return Err(NotCanonical::Empty),
            Some(b'/') => {}
            Some(_) => return Err(NotCanonical::Start),
        }
        let (path, query) = match text.split_once('?') {
            Some((path, query)) => (path, Some(query)),
            None => (text, None),
        };
        Self::walk(path)?;
        if let Some(query) = query {
            Self::pairs(query, path.len() + 1)?;
        }
        Ok(Self(text.to_owned()))
    }

    /// The target as sent.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The path, before any `?`.
    pub fn path(&self) -> &str {
        self.0.split_once('?').map_or(self.0.as_str(), |(path, _)| path)
    }

    /// The query, after the `?`, when there is one.
    pub fn query(&self) -> Option<&str> {
        self.0.split_once('?').map(|(_, query)| query)
    }

    /// Whether `byte` is never escaped.
    const fn unreserved(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
    }

    /// The characters of `piece`, which starts at offset `base`, checked against the alphabet.
    fn alphabet(piece: &str, base: usize) -> Result<(), NotCanonical> {
        let bytes = piece.as_bytes();
        let mut at = 0;
        while let Some(&byte) = bytes.get(at) {
            let here = base + at;
            if byte == b'%' {
                let digit = |offset: usize| bytes.get(at + offset).copied().filter(|d| Crockford::hex(*d).is_some());
                let (Some(high), Some(low)) = (digit(1), digit(2)) else {
                    return Err(NotCanonical::Escape { at: here });
                };
                let value = Crockford::hex(high).unwrap_or_default() << 4 | Crockford::hex(low).unwrap_or_default();
                if Self::unreserved(value) {
                    return Err(NotCanonical::Unreserved { at: here });
                }
                if value == b'/' {
                    return Err(NotCanonical::Delimiter { at: here });
                }
                at += 3;
            } else if Self::unreserved(byte) || byte == b'/' {
                at += 1;
            } else {
                return Err(NotCanonical::Character { at: here });
            }
        }
        Ok(())
    }

    fn walk(path: &str) -> Result<(), NotCanonical> {
        Self::alphabet(path, 0)?;
        let mut at = 1;
        if path.len() == 1 {
            return Ok(());
        }
        for segment in path.get(1..).unwrap_or_default().split('/') {
            if segment.is_empty() {
                return Err(NotCanonical::Segment { at });
            }
            if segment == "." || segment == ".." {
                return Err(NotCanonical::Dot { at });
            }
            at += segment.len() + 1;
        }
        Ok(())
    }

    fn pairs(query: &str, base: usize) -> Result<(), NotCanonical> {
        let mut at = base;
        let mut previous: Option<(&str, &str)> = None;
        for pair in query.split('&') {
            if pair.is_empty() {
                return Err(NotCanonical::Pair { at });
            }
            let (name, value) = pair.split_once('=').ok_or(NotCanonical::Equals { at })?;
            if name.is_empty() {
                return Err(NotCanonical::Name { at });
            }
            Self::alphabet(name, at)?;
            Self::alphabet(value, at + name.len() + 1)?;
            if previous.is_some_and(|before| before > (name, value)) {
                return Err(NotCanonical::Order { at });
            }
            previous = Some((name, value));
            at += pair.len() + 1;
        }
        Ok(())
    }
}

impl fmt::Display for Target {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn canonical_targets_pass_unchanged() {
        for text in ["/", "/v1/sites", "/v1/sites/ab/deploys?limit=5&site=a", "/a%20b/c~d", "/x?a=%20&a=1&b="] {
            assert_eq!(Target::parse(text).map(|target| target.as_str().to_owned()), Ok(text.to_owned()), "{text}");
        }
        let target = Target::parse("/v1/sites?a=1").unwrap();
        assert_eq!((target.path(), target.query()), ("/v1/sites", Some("a=1")));
    }

    #[test]
    fn everything_else_is_refused_with_its_reason() {
        let refuse = |text: &str| Target::parse(text).unwrap_err();
        assert_eq!(refuse(""), NotCanonical::Empty);
        assert_eq!(refuse("v1"), NotCanonical::Start);
        assert_eq!(refuse("/v1/sites/"), NotCanonical::Segment { at: 10 });
        assert_eq!(refuse("/a//b"), NotCanonical::Segment { at: 3 });
        assert_eq!(refuse("/a/./b"), NotCanonical::Dot { at: 3 });
        assert_eq!(refuse("/a/../b"), NotCanonical::Dot { at: 3 });
        assert_eq!(refuse("/a/%7E"), NotCanonical::Unreserved { at: 3 });
        assert_eq!(refuse("/a/%7e"), NotCanonical::Escape { at: 3 });
        assert_eq!(refuse("/a%2Fb"), NotCanonical::Delimiter { at: 2 });
        assert_eq!(refuse("/a%e9"), NotCanonical::Escape { at: 2 });
        assert_eq!(refuse("/a%"), NotCanonical::Escape { at: 2 });
        assert_eq!(refuse("/a b"), NotCanonical::Character { at: 2 });
        assert_eq!(refuse("/a#f"), NotCanonical::Character { at: 2 });
        assert_eq!(refuse("/a?x=1?y"), NotCanonical::Character { at: 6 });
        assert_eq!(refuse("/a?"), NotCanonical::Pair { at: 3 });
        assert_eq!(refuse("/a?x=1&&y=2"), NotCanonical::Pair { at: 7 });
        assert_eq!(refuse("/a?x"), NotCanonical::Equals { at: 3 });
        assert_eq!(refuse("/a?=1"), NotCanonical::Name { at: 3 });
        assert_eq!(refuse("/a?b=1&a=2"), NotCanonical::Order { at: 7 });
        assert_eq!(refuse("/a?a=2&a=1"), NotCanonical::Order { at: 7 });
        assert_eq!(refuse("/a?a=1=2"), NotCanonical::Character { at: 6 });
    }

    #[test]
    fn no_accepted_target_holds_a_line_break_or_a_dot_segment() {
        let alphabet: Vec<char> = "/?&=%.~aA0F2e7 \n#\\\u{e9}".chars().collect();
        let mut state = 0x2545_f491_u64;
        for _ in 0..20_000 {
            let mut text = String::from("/");
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let length = usize::try_from(state >> 60).unwrap_or(0);
            for _ in 0..length {
                state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                let pick = usize::try_from(state >> 33).unwrap_or(0) % alphabet.len();
                text.push(alphabet.get(pick).copied().unwrap_or('/'));
            }
            if let Ok(target) = Target::parse(&text) {
                assert!(!text.contains('\n') && !text.contains(' '), "{text:?}");
                assert!(target.path().split('/').skip(1).all(|segment| segment != "." && segment != ".."), "{text:?}");
            }
        }
    }
}
