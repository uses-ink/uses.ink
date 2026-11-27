//! `Range: bytes=...` for media: one span of a file, or the whole file.

use std::fmt;

use crate::condition::ConditionError;

/// A run of bytes of a blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// The first byte.
    pub start: u64,
    /// How many bytes.
    pub length: u64,
}

impl Span {
    /// All `length` bytes.
    pub const fn whole(length: u64) -> Self {
        Self { start: 0, length }
    }

    /// The last byte's offset, for `Content-Range`. `None` for no bytes.
    pub fn last(self) -> Option<u64> {
        self.start.checked_add(self.length)?.checked_sub(1)
    }
}

/// One range of a `Range` header: `a-b`, `a-` or `-n`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// `first-last` or `first-`.
    From { first: u64, last: Option<u64> },
    /// `-n`: the last `n` bytes.
    Suffix { length: u64 },
}

/// A `Range` header's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ranges {
    unit: Unit,
    parts: Vec<Part>,
}

/// The unit a `Range` asks in. Only bytes are served.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Bytes,
    Other,
}

/// Why a `Range` is answered with the whole file instead of a span (RFC 9110 lets a server do so).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Ignored {
    /// A unit other than `bytes`.
    Unit,
    /// More than one range: this edge serves one.
    Multiple,
    /// The `If-Range` validator no longer names the file.
    Validator,
}

impl fmt::Display for Ignored {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Unit => "range in a unit other than bytes",
            Self::Multiple => "more than one range",
            Self::Validator => "if-range validator does not match",
        })
    }
}

/// What a `Range` comes to for a file of a known length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Serve the span with `206`.
    Span(Span),
    /// Serve everything with `200`, the range ignored for this reason.
    Whole(Ignored),
    /// Nothing the client asked for exists: `416`.
    Unsatisfiable,
}

impl Ranges {
    /// Reads the header's value.
    pub(crate) fn read(value: &[u8]) -> Result<Self, ConditionError> {
        let bad = |at| ConditionError::Range { at };
        let text = std::str::from_utf8(value).map_err(|_| bad(0))?;
        let (unit, specs) = text.trim_start_matches([' ', '\t']).split_once('=').ok_or_else(|| bad(0))?;
        let unit = if unit.eq_ignore_ascii_case("bytes") { Unit::Bytes } else { Unit::Other };
        if unit == Unit::Other {
            return Ok(Self { unit, parts: Vec::new() });
        }
        let mut parts = Vec::new();
        let mut at = text.len() - specs.len();
        for spec in specs.split(',') {
            parts.push(Part::read(spec.trim_matches([' ', '\t'])).ok_or_else(|| bad(at))?);
            at += spec.len() + 1;
        }
        Ok(Self { unit, parts })
    }

    /// What this range comes to against a file of `length` bytes.
    pub fn pick(&self, length: u64) -> Pick {
        if self.unit == Unit::Other {
            return Pick::Whole(Ignored::Unit);
        }
        let [part] = self.parts.as_slice() else { return Pick::Whole(Ignored::Multiple) };
        let Some(last_byte) = length.checked_sub(1) else { return Pick::Unsatisfiable };
        let (start, end) = match *part {
            Part::From { first, .. } if first > last_byte => return Pick::Unsatisfiable,
            Part::From { first, last } => (first, last.map_or(last_byte, |last| last.min(last_byte))),
            Part::Suffix { length: 0 } => return Pick::Unsatisfiable,
            Part::Suffix { length: wanted } => (length - wanted.min(length), last_byte),
        };
        Pick::Span(Span { start, length: end - start + 1 })
    }
}

impl Part {
    fn read(spec: &str) -> Option<Self> {
        let number = |text: &str| -> Option<u64> {
            (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse().ok()).flatten()
        };
        let (first, last) = spec.split_once('-')?;
        match (first, last) {
            ("", last) => Some(Self::Suffix { length: number(last)? }),
            (first, "") => Some(Self::From { first: number(first)?, last: None }),
            (first, last) => {
                let (first, last) = (number(first)?, number(last)?);
                (first <= last).then_some(Self::From { first, last: Some(last) })
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn pick(header: &str, length: u64) -> Pick {
        Ranges::read(header.as_bytes()).unwrap().pick(length)
    }

    fn span(start: u64, length: u64) -> Pick {
        Pick::Span(Span { start, length })
    }

    #[test]
    fn picks_a_span() {
        assert_eq!(pick("bytes=0-9", 100), span(0, 10));
        assert_eq!(pick("bytes=90-", 100), span(90, 10));
        assert_eq!(pick("bytes=-10", 100), span(90, 10));
        assert_eq!(pick("bytes=-500", 100), span(0, 100));
        assert_eq!(pick("bytes=50-5000", 100), span(50, 50));
        assert_eq!(pick("BYTES=0-0", 1), span(0, 1));
    }

    #[test]
    fn says_when_nothing_is_there() {
        assert_eq!(pick("bytes=100-", 100), Pick::Unsatisfiable);
        assert_eq!(pick("bytes=-0", 100), Pick::Unsatisfiable);
        assert_eq!(pick("bytes=0-", 0), Pick::Unsatisfiable);
    }

    #[test]
    fn ignores_what_it_does_not_serve() {
        assert_eq!(pick("bytes=0-1,5-6", 100), Pick::Whole(Ignored::Multiple));
        assert_eq!(pick("items=0-1", 100), Pick::Whole(Ignored::Unit));
    }

    #[test]
    fn refuses_a_malformed_range() {
        for bad in [
            "bytes",
            "bytes=",
            "bytes=a-b",
            "bytes=5-1",
            "bytes=1-2-3",
            "bytes=-",
            "bytes=18446744073709551616-",
            "bytes=0-1,",
        ] {
            assert!(Ranges::read(bad.as_bytes()).is_err(), "{bad}");
        }
    }
}
