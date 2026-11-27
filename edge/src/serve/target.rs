//! The request target: checked byte by byte, decoded once, normalised, never trusted.

use std::fmt;

use model::{Route, RouteError};

use crate::host::{Authority, HostnameError};
use crate::limits::Limits;

/// A request target read into the path it names: percent-decoded once, a trailing `/` dropped, the
/// query ignored. A target that could climb out of the site or name two things is refused, never
/// repaired.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    authority: Option<Authority>,
    path: String,
}

/// Why a request target is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TargetError {
    /// Longer than the operator's limit.
    TooLong {
        /// Its length in bytes.
        length: usize,
        /// The limit.
        limit: usize,
    },
    /// Neither a path starting with `/` nor an `http://` or `https://` URL.
    Form,
    /// A byte outside visible ASCII, a `#` or a `\`, at this offset.
    Byte {
        /// The byte offset.
        at: usize,
        /// The byte.
        found: u8,
    },
    /// A `%` not followed by two hex digits, at this offset.
    Escape {
        /// The byte offset.
        at: usize,
    },
    /// `%2f` or `%5c`, which would hide a separator inside a segment, at this offset.
    Separator {
        /// The byte offset.
        at: usize,
    },
    /// An escape that decodes to NUL or another control byte, at this offset.
    Control {
        /// The byte offset.
        at: usize,
    },
    /// A `%` left after decoding once, as in `%252e`: an escape escaped twice, at this offset.
    Double {
        /// The byte offset of the segment.
        at: usize,
    },
    /// The decoded bytes are not UTF-8.
    Encoding,
    /// Segment number `index` is `.` or `..`, written plainly or escaped.
    Dot {
        /// The segment, from zero.
        index: usize,
    },
    /// Segment number `index` is empty: `//` somewhere in the path.
    Empty {
        /// The segment, from zero.
        index: usize,
    },
    /// More segments than the limit.
    Deep {
        /// How many.
        count: usize,
        /// The limit.
        limit: usize,
    },
    /// The authority of an absolute-form target is no host.
    Authority(HostnameError),
}

impl fmt::Display for TargetError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { length, limit } => write!(out, "the target is {length} bytes, over the limit of {limit}"),
            Self::Form => write!(out, "the target is neither a path nor an http or https URL"),
            Self::Byte { at, found } => write!(out, "byte {found:#04x} at offset {at} may not appear in a target"),
            Self::Escape { at } => write!(out, "the `%` at offset {at} is not followed by two hex digits"),
            Self::Separator { at } => write!(out, "the escape at offset {at} is a path separator"),
            Self::Control { at } => write!(out, "the escape at offset {at} is a control byte"),
            Self::Double { at } => write!(out, "the segment at offset {at} still holds a `%` after decoding"),
            Self::Encoding => write!(out, "the decoded path is not UTF-8"),
            Self::Dot { index } => write!(out, "segment {} of the path is a dot segment", index + 1),
            Self::Empty { index } => write!(out, "segment {} of the path is empty", index + 1),
            Self::Deep { count, limit } => write!(out, "the path has {count} segments, over the limit of {limit}"),
            Self::Authority(_) => write!(out, "the authority of the target is not a host"),
        }
    }
}

impl std::error::Error for TargetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authority(source) => Some(source),
            _ => None,
        }
    }
}

/// One segment of the raw path and where it starts.
#[derive(Clone, Copy, Debug)]
struct Segment<'a> {
    raw: &'a str,
    at: usize,
}

impl Segment<'_> {
    /// The segment's bytes with each escape decoded once.
    fn decode(self) -> Result<String, TargetError> {
        let bytes = self.raw.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut cursor = 0;
        while let Some(&byte) = bytes.get(cursor) {
            if byte != b'%' {
                out.push(byte);
                cursor += 1;
                continue;
            }
            let at = self.at + cursor;
            let digits = bytes.get(cursor + 1..cursor + 3).filter(|pair| pair.iter().all(u8::is_ascii_hexdigit));
            let value = digits
                .and_then(|pair| std::str::from_utf8(pair).ok())
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or(TargetError::Escape { at })?;
            match value {
                b'/' | b'\\' => return Err(TargetError::Separator { at }),
                0..=0x1f | 0x7f => return Err(TargetError::Control { at }),
                _ => out.push(value),
            }
            cursor += 3;
        }
        let text = String::from_utf8(out).map_err(|_| TargetError::Encoding)?;
        if text.contains('%') {
            return Err(TargetError::Double { at: self.at });
        }
        Ok(text)
    }
}

impl Target {
    /// Reads `raw` as received: `/a/b?q` or `http://host/a/b`.
    pub fn parse(raw: &str, limits: &Limits) -> Result<Self, TargetError> {
        let limit = usize::try_from(limits.target.count()).unwrap_or(usize::MAX);
        if raw.len() > limit {
            return Err(TargetError::TooLong { length: raw.len(), limit });
        }
        let visible = |byte: &u8| (0x21..=0x7e).contains(byte) && *byte != b'#' && *byte != b'\\';
        if let Some(at) = raw.bytes().position(|byte| !visible(&byte)) {
            return Err(TargetError::Byte { at, found: raw.as_bytes().get(at).copied().unwrap_or_default() });
        }
        let (authority, offset, rest) = Self::split(raw)?;
        let path = rest.split_once('?').map_or(rest, |(path, _)| path);
        let path = path.strip_prefix('/').unwrap_or_default();
        let mut segments: Vec<&str> = if path.is_empty() { Vec::new() } else { path.split('/').collect() };
        if segments.last() == Some(&"") {
            segments.pop();
        }
        if segments.len() > limits.segments {
            return Err(TargetError::Deep { count: segments.len(), limit: limits.segments });
        }
        let mut at = offset + 1;
        let mut decoded = String::new();
        for (index, raw) in segments.into_iter().enumerate() {
            if raw.is_empty() {
                return Err(TargetError::Empty { index });
            }
            let segment = Segment { raw, at }.decode()?;
            if segment == "." || segment == ".." {
                return Err(TargetError::Dot { index });
            }
            decoded.push('/');
            decoded.push_str(&segment);
            at += raw.len() + 1;
        }
        if decoded.is_empty() {
            decoded.push('/');
        }
        Ok(Self { authority, path: decoded })
    }

    /// The authority of an absolute-form target, the offset where the path starts and the rest.
    fn split(raw: &str) -> Result<(Option<Authority>, usize, &str), TargetError> {
        if raw.starts_with('/') {
            return Ok((None, 0, raw));
        }
        let prefix = ["http://", "https://"]
            .into_iter()
            .find(|prefix| raw.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix)))
            .ok_or(TargetError::Form)?;
        let rest = raw.get(prefix.len()..).unwrap_or_default();
        let end = rest.find(['/', '?']).unwrap_or(rest.len());
        let (host, path) = rest.split_at(end);
        let authority = Authority::try_from(host.as_bytes()).map_err(TargetError::Authority)?;
        let path = if path.starts_with('/') { path } else { "/" };
        Ok((Some(authority), prefix.len() + end, path))
    }

    /// The authority of an absolute-form target.
    pub fn authority(&self) -> Option<&Authority> {
        self.authority.as_ref()
    }

    /// The decoded path: `/`, or `/posts/on-shelves`, with no trailing `/`.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The route the path names, or why it names none (it holds a capital, a space, a `_`).
    pub fn route(&self) -> Result<Route, RouteError> {
        Route::parse(self.path.as_str())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse(raw: &str) -> Result<Target, TargetError> {
        Target::parse(raw, &Limits::default())
    }

    fn path(raw: &str) -> String {
        parse(raw).unwrap().path
    }

    #[test]
    fn normalises_what_is_safe() {
        assert_eq!(path("/"), "/");
        assert_eq!(path("/a/b"), "/a/b");
        assert_eq!(path("/a/b/"), "/a/b");
        assert_eq!(path("/a?x=%zz"), "/a");
        assert_eq!(path("/th%C3%A9"), "/thé");
        assert_eq!(path("http://Example.com/a?x"), "/a");
        assert_eq!(path("http://example.com"), "/");
        assert_eq!(parse("http://example.com:81/").unwrap().authority().unwrap().port, Some(81));
    }

    #[test]
    fn refuses_what_could_name_two_things_or_climb() {
        assert_eq!(parse("/a/../b"), Err(TargetError::Dot { index: 1 }));
        assert_eq!(parse("/%2e%2e/b"), Err(TargetError::Dot { index: 0 }));
        assert_eq!(parse("/a/./b"), Err(TargetError::Dot { index: 1 }));
        assert_eq!(parse("/a%2fb"), Err(TargetError::Separator { at: 2 }));
        assert_eq!(parse("/a%5Cb"), Err(TargetError::Separator { at: 2 }));
        assert_eq!(parse("/a%00"), Err(TargetError::Control { at: 2 }));
        assert_eq!(parse("/%252e%252e/"), Err(TargetError::Double { at: 1 }));
        assert_eq!(parse("/a%zz"), Err(TargetError::Escape { at: 2 }));
        assert_eq!(parse("/%C3"), Err(TargetError::Encoding));
        assert_eq!(parse("//evil.com/"), Err(TargetError::Empty { index: 0 }));
        assert_eq!(parse("/a//b"), Err(TargetError::Empty { index: 1 }));
        assert_eq!(parse("/a//"), Err(TargetError::Empty { index: 1 }));
        assert_eq!(parse("//"), Err(TargetError::Empty { index: 0 }));
        assert_eq!(parse("/a\\b"), Err(TargetError::Byte { at: 2, found: b'\\' }));
        assert_eq!(parse("/a b"), Err(TargetError::Byte { at: 2, found: b' ' }));
        assert_eq!(parse("/a#b"), Err(TargetError::Byte { at: 2, found: b'#' }));
        assert_eq!(parse("*"), Err(TargetError::Form));
        assert_eq!(parse("a/b"), Err(TargetError::Form));
        assert_eq!(parse("ftp://a/b"), Err(TargetError::Form));
    }

    #[test]
    fn refuses_what_is_too_big() {
        assert!(matches!(parse(&format!("/{}", "a".repeat(5000))), Err(TargetError::TooLong { limit: 4096, .. })));
        assert!(matches!(parse(&"/a".repeat(40)), Err(TargetError::Deep { count: 40, limit: 32 })));
    }
}
