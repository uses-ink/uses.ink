//! Email addresses.

use std::fmt;
use std::str::FromStr;

/// The longest address, in bytes (RFC 5321).
const LONGEST: usize = 254;
/// The longest part before the `@`.
const LOCAL: usize = 64;
/// The longest label of the domain.
const LABEL: usize = 63;

/// An email address: one `@`, a local part of the characters mail systems accept without
/// quoting, and a domain of letters, digits and inner hyphens. It is kept in lower case, so two
/// spellings of one address are one value; that is how every provider the product mails treats
/// them.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address(String);

impl Address {
    /// The address as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Address {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Address({})", self.0)
    }
}

/// Text that is no address, and the first fact that makes it wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AddressError {
    /// Nothing was written.
    Empty,
    /// Longer than 254 bytes.
    Long {
        /// How long.
        found: usize,
    },
    /// Not exactly one `@`.
    At {
        /// How many there are.
        found: usize,
    },
    /// The part before the `@` is empty, over 64 bytes, or starts, ends or doubles a dot.
    Local,
    /// The domain is empty, or has a label that is empty, over 63 bytes or starts or ends with
    /// a hyphen.
    Domain,
    /// A character that is not allowed there, at this byte offset.
    Character {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for AddressError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("the address is empty"),
            Self::Long { found } => write!(out, "the address is {found} bytes, over {LONGEST}"),
            Self::At { found } => write!(out, "an address has one `@`, this has {found}"),
            Self::Local => out.write_str("the part before the `@` is not a usable local part"),
            Self::Domain => out.write_str("the part after the `@` is not a usable domain"),
            Self::Character { at } => write!(out, "byte {at} of the address is not allowed there"),
        }
    }
}

impl std::error::Error for AddressError {}

impl Address {
    /// What the local part may hold besides letters and digits.
    const SPECIAL: &'static str = ".!#$%&'*+/=?^_`{|}~-";

    fn local(part: &str) -> Result<(), AddressError> {
        if part.is_empty() || part.len() > LOCAL || part.starts_with('.') || part.ends_with('.') || part.contains("..")
        {
            return Err(AddressError::Local);
        }
        match part.bytes().position(|b| !(b.is_ascii_alphanumeric() || Self::SPECIAL.as_bytes().contains(&b))) {
            Some(at) => Err(AddressError::Character { at }),
            None => Ok(()),
        }
    }

    fn domain(part: &str, from: usize) -> Result<(), AddressError> {
        if part.is_empty() {
            return Err(AddressError::Domain);
        }
        for label in part.split('.') {
            if label.is_empty() || label.len() > LABEL || label.starts_with('-') || label.ends_with('-') {
                return Err(AddressError::Domain);
            }
        }
        match part.bytes().position(|b| !(b.is_ascii_alphanumeric() || b == b'-' || b == b'.')) {
            Some(at) => Err(AddressError::Character { at: from + at }),
            None => Ok(()),
        }
    }
}

impl FromStr for Address {
    type Err = AddressError;

    fn from_str(text: &str) -> Result<Self, AddressError> {
        if text.is_empty() {
            return Err(AddressError::Empty);
        }
        if text.len() > LONGEST {
            return Err(AddressError::Long { found: text.len() });
        }
        let found = text.matches('@').count();
        let (Some((local, domain)), 1) = (text.split_once('@'), found) else {
            return Err(AddressError::At { found });
        };
        Self::local(local)?;
        Self::domain(domain, local.len() + 1)?;
        Ok(Self(text.to_ascii_lowercase()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn reads_an_address_in_lower_case() {
        assert_eq!("Me.Name+tag@Example.ORG".parse::<Address>().unwrap().as_str(), "me.name+tag@example.org");
        assert!("me@localhost".parse::<Address>().is_ok());
    }

    #[test]
    fn refuses_what_is_no_address_and_says_why() {
        let read = |text: &str| text.parse::<Address>().unwrap_err();
        assert_eq!(read(""), AddressError::Empty);
        assert_eq!(read("me"), AddressError::At { found: 0 });
        assert_eq!(read("a@b@c"), AddressError::At { found: 2 });
        assert_eq!(read("@x.org"), AddressError::Local);
        assert_eq!(read(".a@x.org"), AddressError::Local);
        assert_eq!(read("a..b@x.org"), AddressError::Local);
        assert_eq!(read("a@"), AddressError::Domain);
        assert_eq!(read("a@-x.org"), AddressError::Domain);
        assert_eq!(read("a@x..org"), AddressError::Domain);
        assert_eq!(read("a b@x.org"), AddressError::Character { at: 1 });
        assert_eq!(read("a@x_y.org"), AddressError::Character { at: 3 });
        assert_eq!(read("a<b>@x.org"), AddressError::Character { at: 1 });
        assert_eq!(read(&format!("{}@x.org", "a".repeat(300))), AddressError::Long { found: 306 });
        assert_eq!(read("a\r\nbcc:x@x.org"), AddressError::Character { at: 1 });
    }
}
