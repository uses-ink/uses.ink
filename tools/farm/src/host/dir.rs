//! Where the farm keeps its files on a host.

use std::fmt;
use std::str::FromStr;

/// A directory on a host: absolute, or relative to the home of the user that logs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostDir(String);

impl HostDir {
    /// The default, `~/farm`.
    pub(crate) fn standard() -> Self {
        Self("farm".to_owned())
    }

    /// The path of `parts` below it, with `/` between.
    pub(crate) fn join(&self, parts: &[&str]) -> String {
        let mut path = self.0.clone();
        for part in parts {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
        }
        path
    }

    /// The lock file of slot `n`.
    #[cfg(test)]
    pub(crate) fn slot(&self, n: u32) -> String {
        self.join(&["slots", &format!("{n}.lock")])
    }

    /// The directory as an argument to a program.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for HostDir {
    type Err = DirFault;

    fn from_str(given: &str) -> Result<Self, DirFault> {
        let path = match given {
            "~" => "",
            other => other.strip_prefix("~/").unwrap_or(other),
        };
        let bad = |why| DirFault { given: given.to_owned(), why };
        if path.is_empty() && given != "~" {
            return Err(bad(Why::Empty));
        }
        if let Some(found) = path.chars().find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))) {
            return Err(bad(Why::Char(found)));
        }
        if path.split('/').any(|part| part == ".." || part == ".") {
            return Err(bad(Why::Dots));
        }
        Ok(Self(path.trim_end_matches('/').to_owned()))
    }
}

impl fmt::Display for HostDir {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_str() {
            "" => out.write_str("~"),
            path if path.starts_with('/') => out.write_str(path),
            path => write!(out, "~/{path}"),
        }
    }
}

/// A host directory that is not allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirFault {
    given: String,
    why: Why,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Why {
    Empty,
    Char(char),
    Dots,
}

impl fmt::Display for DirFault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let given = &self.given;
        match self.why {
            Why::Empty => write!(out, "the host directory is empty"),
            Why::Char(found) => {
                write!(out, "host directory `{given}` holds {found:?}; use letters, digits and `/ . _ -`")
            }
            Why::Dots => write!(out, "host directory `{given}` holds a `.` or `..` part"),
        }
    }
}

impl std::error::Error for DirFault {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_are_home_relative_or_absolute_and_never_dotted() {
        assert_eq!("~/farm".parse::<HostDir>().unwrap().join(&["slots", "0.lock"]), "farm/slots/0.lock");
        assert_eq!("/srv/farm/".parse::<HostDir>().unwrap().slot(3), "/srv/farm/slots/3.lock");
        assert_eq!("~".parse::<HostDir>().unwrap().join(&["x"]), "x");
        for bad in ["", "a b", "../x", "a/../b", "a;b", "$HOME", "~/a'b"] {
            assert!(bad.parse::<HostDir>().is_err(), "{bad:?}");
        }
        assert_eq!("~/farm".parse::<HostDir>().unwrap().to_string(), "~/farm");
    }
}
