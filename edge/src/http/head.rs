//! A request's header lines, as the adapter hands them over: bytes, checked before they are read.

use std::fmt;

use crate::limits::Limits;

/// A header that may appear once. Two of them are two requests to a proxy and one to us.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Singular {
    /// `Host`.
    Host,
    /// `Range`.
    Range,
    /// `If-Range`.
    IfRange,
    /// `If-None-Match`.
    IfNoneMatch,
    /// `Accept-Encoding`.
    AcceptEncoding,
}

impl Singular {
    /// The name as HTTP/2 writes it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Range => "range",
            Self::IfRange => "if-range",
            Self::IfNoneMatch => "if-none-match",
            Self::AcceptEncoding => "accept-encoding",
        }
    }
}

impl fmt::Display for Singular {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// Why a request's headers are refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HeaderError {
    /// More headers than the limit.
    Count {
        /// How many.
        count: usize,
        /// The limit.
        limit: usize,
    },
    /// Header number `index` has a value over the limit.
    Size {
        /// The header, from zero.
        index: usize,
        /// Its value's length in bytes.
        length: usize,
        /// The limit.
        limit: usize,
    },
    /// All the headers together are over the limit.
    Total {
        /// Their length in bytes.
        length: usize,
        /// The limit.
        limit: usize,
    },
    /// Header number `index` has a name that is not an HTTP token.
    Name {
        /// The header, from zero.
        index: usize,
    },
    /// Header number `index` holds a CR, an LF or a NUL at this offset of its value.
    Byte {
        /// The header, from zero.
        index: usize,
        /// The byte offset in its value.
        at: usize,
    },
    /// This header appears more than once.
    Repeated {
        /// Which.
        name: Singular,
    },
}

impl fmt::Display for HeaderError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count { count, limit } => write!(out, "the request has {count} headers, over the limit of {limit}"),
            Self::Size { index, length, limit } => {
                write!(out, "header {} is {length} bytes, over the limit of {limit}", index + 1)
            }
            Self::Total { length, limit } => write!(out, "the headers are {length} bytes, over the limit of {limit}"),
            Self::Name { index } => write!(out, "header {} has a name that is not a token", index + 1),
            Self::Byte { index, at } => write!(out, "header {} holds a line break or NUL at offset {at}", index + 1),
            Self::Repeated { name } => write!(out, "the request has more than one `{name}` header"),
        }
    }
}

impl std::error::Error for HeaderError {}

/// Why a request that is only read carries a body, or says it might.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BodyError {
    /// A `Content-Length` above zero.
    Declared {
        /// The length it says.
        length: u64,
    },
    /// A `Content-Length` that is not a decimal number.
    Invalid,
    /// A `Transfer-Encoding`.
    Chunked,
    /// Both a `Content-Length` and a `Transfer-Encoding`: the shape of request smuggling.
    Framing,
}

impl fmt::Display for BodyError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declared { length } => {
                write!(out, "the request declares a body of {length} bytes; GET and HEAD have none")
            }
            Self::Invalid => write!(out, "the content-length is not a decimal number"),
            Self::Chunked => write!(out, "the request has a transfer-encoding; GET and HEAD have no body"),
            Self::Framing => write!(out, "the request has both content-length and transfer-encoding"),
        }
    }
}

impl std::error::Error for BodyError {}

/// The header lines of one request, names and values as bytes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Head<'a, 'r>(pub(crate) &'r [(&'a [u8], &'a [u8])]);

impl<'a> Head<'a, '_> {
    /// Checks the lines against the limits and for bytes that end a line.
    pub(crate) fn check(&self, limits: &Limits) -> Result<(), HeaderError> {
        if self.0.len() > limits.headers {
            return Err(HeaderError::Count { count: self.0.len(), limit: limits.headers });
        }
        let each = usize::try_from(limits.header.count()).unwrap_or(usize::MAX);
        let all = usize::try_from(limits.head.count()).unwrap_or(usize::MAX);
        let mut total = 0usize;
        for (index, (name, value)) in self.0.iter().enumerate() {
            let token = |byte: &u8| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(byte);
            if name.is_empty() || !name.iter().all(token) {
                return Err(HeaderError::Name { index });
            }
            if value.len() > each {
                return Err(HeaderError::Size { index, length: value.len(), limit: each });
            }
            if let Some(at) = value.iter().position(|byte| matches!(byte, b'\r' | b'\n' | 0)) {
                return Err(HeaderError::Byte { index, at });
            }
            total = total.saturating_add(name.len()).saturating_add(value.len());
            if total > all {
                return Err(HeaderError::Total { length: total, limit: all });
            }
        }
        Ok(())
    }

    /// The values of the header `name`, in order.
    pub(crate) fn all(&self, name: &'static str) -> impl Iterator<Item = &'a [u8]> {
        self.0.iter().filter(move |(header, _)| header.eq_ignore_ascii_case(name.as_bytes())).map(|(_, value)| *value)
    }

    /// The one value of a header that may appear once, `None` when it does not appear.
    pub(crate) fn one(&self, name: Singular) -> Result<Option<&'a [u8]>, HeaderError> {
        let mut values = self.all(name.name());
        let first = values.next();
        if values.next().is_some() {
            return Err(HeaderError::Repeated { name });
        }
        Ok(first)
    }

    /// Refuses a body, or a header that says one may follow.
    pub(crate) fn bodiless(&self) -> Result<(), BodyError> {
        let chunked = self.all("transfer-encoding").next().is_some();
        let mut lengths = self.all("content-length").peekable();
        if chunked && lengths.peek().is_some() {
            return Err(BodyError::Framing);
        }
        if chunked {
            return Err(BodyError::Chunked);
        }
        for value in lengths {
            let text = value.trim_ascii();
            if text.is_empty() || !text.iter().all(u8::is_ascii_digit) {
                return Err(BodyError::Invalid);
            }
            let length: u64 =
                std::str::from_utf8(text).ok().and_then(|text| text.parse().ok()).ok_or(BodyError::Invalid)?;
            if length > 0 {
                return Err(BodyError::Declared { length });
            }
        }
        Ok(())
    }
}
