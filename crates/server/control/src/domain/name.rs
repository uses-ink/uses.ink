//! The name of a custom domain.

use std::fmt;
use std::str::FromStr;

use serde::Serialize;

/// A host name a site asks to serve: lower-case ASCII labels of letters, digits and hyphens,
/// joined by dots, at least two of them, at most 253 characters, with the top label not all digits
/// (so no IPv4 address is one). A name with other characters is written in its `xn--` form. A
/// trailing dot is dropped and capitals are lower-cased, so one name has one spelling.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct DomainName(String);

/// Text that is no domain name.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameError {
    /// Nothing is left of it.
    Empty,
    /// Longer than a name may be.
    Long {
        /// The characters it has.
        found: usize,
    },
    /// A label is empty or longer than 63 characters.
    Label {
        /// Which label, counted from 0.
        at: usize,
    },
    /// A character that is not a letter, a digit, a hyphen or a dot.
    Character {
        /// Where, counted in characters from 0.
        at: usize,
    },
    /// A label starts or ends with a hyphen.
    Hyphen {
        /// Which label, counted from 0.
        at: usize,
    },
    /// One label: `localhost` and a bare word are no domain.
    Single,
    /// The top label is all digits, as an IP address is.
    Numeric,
}

impl fmt::Display for NameError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("a domain name is not empty"),
            Self::Long { found } => {
                write!(out, "a domain name is at most {} characters, not {found}", DomainName::LONGEST)
            }
            Self::Label { at } => write!(out, "label {at} of the domain name is empty or over 63 characters"),
            Self::Character { at } => write!(
                out,
                "character {at} of the domain name is not a letter, a digit, a hyphen or a dot; write other characters in their `xn--` form"
            ),
            Self::Hyphen { at } => write!(out, "label {at} of the domain name starts or ends with a hyphen"),
            Self::Single => out.write_str("a domain name has at least two labels, like `example.com`"),
            Self::Numeric => out.write_str("the last label of a domain name is not all digits, so it is no IP address"),
        }
    }
}

impl std::error::Error for NameError {}

impl DomainName {
    /// The most characters a name has.
    pub const LONGEST: usize = 253;

    /// The name as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the name is `parent` or a name under it: `www.example.com` is under `example.com`.
    pub fn is_under(&self, parent: &str) -> bool {
        self.0 == parent || self.0.strip_suffix(parent).is_some_and(|rest| rest.ends_with('.'))
    }
}

impl FromStr for DomainName {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        let text = text.strip_suffix('.').unwrap_or(text).to_ascii_lowercase();
        let found = text.chars().count();
        if found == 0 {
            return Err(NameError::Empty);
        }
        if found > Self::LONGEST {
            return Err(NameError::Long { found });
        }
        if let Some(at) =
            text.chars().position(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.'))
        {
            return Err(NameError::Character { at });
        }
        let labels: Vec<&str> = text.split('.').collect();
        for (at, label) in labels.iter().enumerate() {
            if label.is_empty() || label.len() > 63 {
                return Err(NameError::Label { at });
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(NameError::Hyphen { at });
            }
        }
        if labels.len() < 2 {
            return Err(NameError::Single);
        }
        if labels.last().is_some_and(|top| top.bytes().all(|byte| byte.is_ascii_digit())) {
            return Err(NameError::Numeric);
        }
        Ok(Self(text))
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_name_has_one_spelling() {
        assert_eq!("Blog.Example.COM.".parse::<DomainName>().unwrap().as_str(), "blog.example.com");
        assert_eq!("xn--bcher-kva.example".parse::<DomainName>().unwrap().as_str(), "xn--bcher-kva.example");
    }

    #[test]
    fn text_that_is_no_name_says_how_it_differs() {
        assert_eq!("".parse::<DomainName>(), Err(NameError::Empty));
        assert_eq!("localhost".parse::<DomainName>(), Err(NameError::Single));
        assert_eq!("a..b".parse::<DomainName>(), Err(NameError::Label { at: 1 }));
        assert_eq!("-a.com".parse::<DomainName>(), Err(NameError::Hyphen { at: 0 }));
        assert_eq!("a.com-".parse::<DomainName>(), Err(NameError::Hyphen { at: 1 }));
        assert_eq!("b\u{fc}cher.example".parse::<DomainName>(), Err(NameError::Character { at: 1 }));
        assert_eq!("a b.com".parse::<DomainName>(), Err(NameError::Character { at: 1 }));
        assert_eq!("127.0.0.1".parse::<DomainName>(), Err(NameError::Numeric));
        assert_eq!(format!("{}.com", "a".repeat(64)).parse::<DomainName>(), Err(NameError::Label { at: 0 }));
        assert!(matches!(format!("{}.com", "a.".repeat(130)).parse::<DomainName>(), Err(NameError::Long { .. })));
    }

    #[test]
    fn a_name_is_under_its_parent_by_label() {
        let name: DomainName = "www.example.com".parse().unwrap();
        assert!(name.is_under("example.com") && name.is_under("www.example.com"));
        assert!(!name.is_under("ample.com") && !name.is_under("other.com"));
    }
}
