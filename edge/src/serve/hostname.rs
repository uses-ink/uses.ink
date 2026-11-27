//! The name a request is for, checked before anything looks it up or joins it to a path.

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

/// A host name as a request names it: lower case, without a port or a trailing dot, either a DNS
/// name (letters, digits and `-` in labels of 1 to 63 bytes, 253 in all) or an IP address.
///
/// A name holds no `/`, `\`, `..` or NUL, so a lookup may join it to a folder.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hostname {
    text: String,
    form: Form,
}

/// What a [`Hostname`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Form {
    Name,
    Address,
}

/// A `Host` header or the authority of an absolute-form target: a [`Hostname`] and its port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authority {
    /// The name.
    pub hostname: Hostname,
    /// The port, when the request gave one.
    pub port: Option<u16>,
}

/// Why bytes are no [`Authority`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HostnameError {
    /// There is no host: the header is absent or empty.
    Missing,
    /// Longer than a name can be.
    TooLong {
        /// Its length in bytes.
        length: usize,
        /// The longest allowed.
        limit: usize,
    },
    /// A byte a host does not hold (a space, `@`, `/`, `,`, a non-ASCII byte), at this offset.
    Character {
        /// The byte offset.
        at: usize,
        /// The byte.
        found: u8,
    },
    /// Label number `index` (from zero) is empty, longer than 63 bytes, or starts or ends with `-`.
    Label {
        /// The label, from zero.
        index: usize,
    },
    /// A port that is empty, not decimal or above 65535, at this offset.
    Port {
        /// The byte offset.
        at: usize,
    },
    /// A bracketed address that is no IPv6 address, or text after its `]` that is not a port.
    Literal,
}

impl fmt::Display for HostnameError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(out, "the request names no host"),
            Self::TooLong { length, limit } => write!(out, "the host is {length} bytes, over the limit of {limit}"),
            Self::Character { at, found } => write!(out, "byte {found:#04x} at offset {at} is not part of a host name"),
            Self::Label { index } => write!(out, "label {} of the host is empty, too long, or edged by `-`", index + 1),
            Self::Port { at } => write!(out, "the port at offset {at} is not a number up to 65535"),
            Self::Literal => write!(out, "the bracketed host is not an IPv6 address"),
        }
    }
}

impl std::error::Error for HostnameError {}

impl Hostname {
    /// The longest a name is, as DNS has it.
    pub const LIMIT: usize = 253;

    /// The longest a label is.
    const LABEL: usize = 63;

    /// The host in the form a lookup compares: a DNS name, or an address text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The DNS name, for a lookup that joins it to a path. `None` for an IP address.
    pub fn name(&self) -> Option<&str> {
        (self.form == Form::Name).then_some(self.text.as_str())
    }

    fn dns(text: &str) -> Result<Self, HostnameError> {
        let text = text.strip_suffix('.').unwrap_or(text).to_ascii_lowercase();
        if text.is_empty() {
            return Err(HostnameError::Missing);
        }
        if text.len() > Self::LIMIT {
            return Err(HostnameError::TooLong { length: text.len(), limit: Self::LIMIT });
        }
        let plain = |byte: &u8| byte.is_ascii_alphanumeric() || b".-".contains(byte);
        if let Some(at) = text.as_bytes().iter().position(|byte| !plain(byte)) {
            return Err(HostnameError::Character { at, found: text.as_bytes().get(at).copied().unwrap_or_default() });
        }
        for (index, label) in text.split('.').enumerate() {
            let edged = label.starts_with('-') || label.ends_with('-');
            if label.is_empty() || label.len() > Self::LABEL || edged {
                return Err(HostnameError::Label { index });
            }
        }
        let form = if Ipv4Addr::from_str(&text).is_ok() { Form::Address } else { Form::Name };
        Ok(Self { text, form })
    }

    fn literal(text: &str) -> Result<Self, HostnameError> {
        let address = Ipv6Addr::from_str(text).map_err(|_| HostnameError::Literal)?;
        Ok(Self { text: format!("[{address}]"), form: Form::Address })
    }
}

impl fmt::Display for Hostname {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.text)
    }
}

impl Authority {
    /// The largest an authority is: a name, `:` and five digits.
    const LIMIT: usize = Hostname::LIMIT + 6;

    fn port(raw: &[u8], start: usize) -> Result<u16, HostnameError> {
        let digits = raw.get(start..).unwrap_or_default();
        let bad = || HostnameError::Port { at: start };
        if digits.is_empty() || digits.len() > 5 || !digits.iter().all(u8::is_ascii_digit) {
            return Err(bad());
        }
        let text = std::str::from_utf8(digits).map_err(|_| bad())?;
        text.parse().map_err(|_| bad())
    }
}

impl TryFrom<&[u8]> for Authority {
    type Error = HostnameError;

    /// Reads `name`, `name:port`, `192.0.2.1:port` or `[::1]:port`, in any case. One trailing dot
    /// is dropped. Anything else is refused: a userinfo, a path, a list, spaces.
    fn try_from(raw: &[u8]) -> Result<Self, HostnameError> {
        if raw.is_empty() {
            return Err(HostnameError::Missing);
        }
        if raw.len() > Self::LIMIT {
            return Err(HostnameError::TooLong { length: raw.len(), limit: Self::LIMIT });
        }
        let allowed = |byte: &u8| byte.is_ascii_alphanumeric() || b".-:[]".contains(byte);
        if let Some(at) = raw.iter().position(|byte| !allowed(byte)) {
            return Err(HostnameError::Character { at, found: raw.get(at).copied().unwrap_or_default() });
        }
        let text = std::str::from_utf8(raw).map_err(|_| HostnameError::Literal)?;
        if let Some(inner) = text.strip_prefix('[') {
            let (address, rest) = inner.split_once(']').ok_or(HostnameError::Literal)?;
            let port = match rest.strip_prefix(':') {
                Some(_) => Some(Self::port(raw, address.len() + 3)?),
                None if rest.is_empty() => None,
                None => return Err(HostnameError::Literal),
            };
            return Ok(Self { hostname: Hostname::literal(address)?, port });
        }
        match text.split_once(':') {
            None => Ok(Self { hostname: Hostname::dns(text)?, port: None }),
            Some((name, _)) => {
                let colon = name.len();
                let port = Self::port(raw, colon + 1)?;
                Ok(Self { hostname: Hostname::dns(name)?, port: Some(port) })
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse(text: &str) -> Result<Authority, HostnameError> {
        Authority::try_from(text.as_bytes())
    }

    #[test]
    fn reads_names_ports_and_addresses() {
        let name = parse("Example.COM.:8080").unwrap();
        assert_eq!(name.hostname.as_str(), "example.com");
        assert_eq!(name.hostname.name(), Some("example.com"));
        assert_eq!(name.port, Some(8080));
        assert_eq!(parse("127.0.0.1").unwrap().hostname.name(), None);
        assert_eq!(parse("[::1]:80").unwrap().hostname.as_str(), "[::1]");
        assert_eq!(parse("[::1]:80").unwrap().port, Some(80));
    }

    #[test]
    fn refuses_what_is_no_host() {
        assert_eq!(parse(""), Err(HostnameError::Missing));
        assert_eq!(parse("a b"), Err(HostnameError::Character { at: 1, found: b' ' }));
        assert_eq!(parse("a]b"), Err(HostnameError::Character { at: 1, found: b']' }));
        assert_eq!(parse("[a.com"), Err(HostnameError::Literal));
        assert_eq!(parse("a[b.com:80"), Err(HostnameError::Character { at: 1, found: b'[' }));
        assert_eq!(parse("a@b.com"), Err(HostnameError::Character { at: 1, found: b'@' }));
        assert_eq!(parse("a.com/x"), Err(HostnameError::Character { at: 5, found: b'/' }));
        assert_eq!(parse("a..b"), Err(HostnameError::Label { index: 1 }));
        assert_eq!(parse("-a.com"), Err(HostnameError::Label { index: 0 }));
        assert_eq!(parse(".."), Err(HostnameError::Label { index: 0 }));
        assert_eq!(parse("a.com:"), Err(HostnameError::Port { at: 6 }));
        assert_eq!(parse("a.com:65536"), Err(HostnameError::Port { at: 6 }));
        assert_eq!(parse("[::1"), Err(HostnameError::Literal));
        assert_eq!(parse("[nope]"), Err(HostnameError::Literal));
        assert!(matches!(parse(&"a".repeat(300)), Err(HostnameError::TooLong { .. })));
        assert!(matches!(parse(&format!("{}.com", "a".repeat(64))), Err(HostnameError::Label { index: 0 })));
    }
}
