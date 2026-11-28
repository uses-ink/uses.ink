//! Names: a host, a path segment, a path, a repository.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use url::Url;

/// Text that is no host, segment or path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameError {
    /// Nothing.
    Empty,
    /// Longer than the name may be.
    Long {
        /// How long.
        found: usize,
        /// The most it may be.
        limit: usize,
    },
    /// A character the name does not have, at this byte offset.
    Character {
        /// The offset.
        at: usize,
    },
    /// An empty label, or a segment that is `.` or `..`.
    Shape,
    /// A path of the wrong depth.
    Depth {
        /// How many segments.
        found: usize,
    },
}

impl fmt::Display for NameError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("a name is not empty"),
            Self::Long { found, limit } => write!(out, "a name of {found} bytes is longer than the {limit} allowed"),
            Self::Character { at } => write!(out, "byte {at} of the name is a character it does not have"),
            Self::Shape => out.write_str("a name has no empty label and is not `.` or `..`"),
            Self::Depth { found } => {
                write!(out, "a repository path has {} to {} segments, not {found}", Path::LEAST, Path::MOST)
            }
        }
    }
}

impl std::error::Error for NameError {}

/// The DNS name of a forge, lower-case: `github.com`, `codeberg.org`. No port: an instance is
/// reached on 443.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Host(Cow<'static, str>);

impl Host {
    /// GitHub.
    pub const GITHUB: Self = Self(Cow::Borrowed("github.com"));
    /// GitHub's API.
    pub const API: Self = Self(Cow::Borrowed("api.github.com"));
    /// GitLab's own instance.
    pub const GITLAB: Self = Self(Cow::Borrowed("gitlab.com"));
    /// The longest a DNS name is.
    pub const LIMIT: usize = 253;

    /// The host as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// An `https` URL on this host whose path is `parts`, each percent-encoded as one segment, so
    /// a `/` inside a part stays inside it.
    pub fn url(&self, parts: &[&str]) -> Result<Url, url::ParseError> {
        let mut text = format!("https://{}", self.0);
        for part in parts {
            text.push('/');
            for byte in part.bytes() {
                if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                    text.push(char::from(byte));
                } else {
                    text.push('%');
                    text.extend(
                        [byte >> 4, byte & 15]
                            .into_iter()
                            .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
                            .map(|digit| digit.to_ascii_uppercase()),
                    );
                }
            }
        }
        Url::parse(&text)
    }
}

impl fmt::Display for Host {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl FromStr for Host {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        if text.is_empty() {
            return Err(NameError::Empty);
        }
        if text.len() > Self::LIMIT {
            return Err(NameError::Long { found: text.len(), limit: Self::LIMIT });
        }
        let lower = text.to_ascii_lowercase();
        if let Some(at) = lower.bytes().position(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.')))
        {
            return Err(NameError::Character { at });
        }
        if lower.split('.').any(|label| label.is_empty() || label.starts_with('-') || label.ends_with('-')) {
            return Err(NameError::Shape);
        }
        Ok(Self(Cow::Owned(lower)))
    }
}

impl TryFrom<String> for Host {
    type Error = NameError;

    fn try_from(text: String) -> Result<Self, NameError> {
        text.parse()
    }
}

impl<'de> serde::Deserialize<'de> for Host {
    fn deserialize<D: serde::Deserializer<'de>>(reader: D) -> Result<Self, D::Error> {
        let text = String::deserialize(reader)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// One part of a repository path: an owner, a group or a name. Forges treat these as
/// case-insensitive, so it is kept lower-case.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Segment(String);

impl Segment {
    /// The longest a segment is.
    pub const LIMIT: usize = 100;

    /// The segment as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl FromStr for Segment {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        if text.is_empty() {
            return Err(NameError::Empty);
        }
        if text.len() > Self::LIMIT {
            return Err(NameError::Long { found: text.len(), limit: Self::LIMIT });
        }
        if let Some(at) =
            text.bytes().position(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_')))
        {
            return Err(NameError::Character { at });
        }
        if text == "." || text == ".." {
            return Err(NameError::Shape);
        }
        Ok(Self(text.to_ascii_lowercase()))
    }
}

/// The path of a repository on its forge: `owner/name`, or `group/sub/name` on GitLab.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path {
    owner: Segment,
    groups: Vec<Segment>,
    name: Segment,
}

impl Path {
    /// The fewest segments: an owner and a name.
    pub const LEAST: usize = 2;
    /// The most segments: GitLab allows twenty levels.
    pub const MOST: usize = 20;

    /// The first segment: the user, organization or top-level group.
    pub fn owner(&self) -> &Segment {
        &self.owner
    }

    /// The last segment: the repository.
    pub fn name(&self) -> &Segment {
        &self.name
    }

    /// Every segment, outermost first.
    pub fn segments(&self) -> impl Iterator<Item = &Segment> {
        std::iter::once(&self.owner).chain(&self.groups).chain(std::iter::once(&self.name))
    }
}

impl fmt::Display for Path {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments().enumerate() {
            if index > 0 {
                out.write_str("/")?;
            }
            out.write_str(segment.as_str())?;
        }
        Ok(())
    }
}

impl FromStr for Path {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        let mut segments = text.split('/').map(str::parse).collect::<Result<Vec<Segment>, _>>()?;
        let found = segments.len();
        if !(Self::LEAST..=Self::MOST).contains(&found) {
            return Err(NameError::Depth { found });
        }
        let (Some(name), Some(owner)) = (segments.pop(), (!segments.is_empty()).then(|| segments.remove(0))) else {
            return Err(NameError::Depth { found });
        };
        Ok(Self { owner, groups: segments, name })
    }
}

/// A repository: the forge and the path on it. Two forges that share a path are two repositories.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Repo {
    /// The forge.
    pub host: Host,
    /// The path on it.
    pub path: Path,
}

impl Repo {
    /// The repository at `path` on `host`.
    pub const fn new(host: Host, path: Path) -> Self {
        Self { host, path }
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}/{}", self.host, self.path)
    }
}

impl FromStr for Repo {
    type Err = NameError;

    /// Reads what [`Repo`] prints: the host, a slash and the path.
    fn from_str(text: &str) -> Result<Self, NameError> {
        let (host, path) = text.split_once('/').ok_or(NameError::Depth { found: 0 })?;
        Ok(Self { host: host.parse()?, path: path.parse()? })
    }
}

impl Repo {
    /// The repository an `https` address names: `https://host/owner/name`, with a trailing `.git`
    /// and slash left off. Anything else (another scheme, a port, a user, a query) is no address of
    /// a repository on a forge, and says why.
    pub fn at(address: &str) -> Result<Self, NameError> {
        let rest = address.strip_prefix("https://").ok_or(NameError::Shape)?;
        let rest = rest.trim_end_matches('/');
        let rest = rest.strip_suffix(".git").unwrap_or(rest);
        rest.parse()
    }
}

impl serde::Serialize for Repo {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Repo {
    fn deserialize<D: serde::Deserializer<'de>>(reader: D) -> Result<Self, D::Error> {
        let text = String::deserialize(reader)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn names_are_lower_case_and_bounded() {
        assert_eq!("Octo/Docs".parse::<Path>().unwrap().to_string(), "octo/docs");
        assert_eq!("a/b/c/d".parse::<Path>().unwrap().owner().as_str(), "a");
        assert_eq!("solo".parse::<Path>(), Err(NameError::Depth { found: 1 }));
        assert_eq!("a/../b".parse::<Path>(), Err(NameError::Shape));
        assert_eq!("a//b".parse::<Path>(), Err(NameError::Empty));
        assert_eq!("a/b c".parse::<Path>(), Err(NameError::Character { at: 1 }));
        assert_eq!("Codeberg.ORG".parse::<Host>().unwrap().as_str(), "codeberg.org");
        assert_eq!("a..b".parse::<Host>(), Err(NameError::Shape));
        assert_eq!("a b".parse::<Host>(), Err(NameError::Character { at: 1 }));
    }

    #[test]
    fn a_repo_reads_what_it_prints_and_an_address() {
        let repo: Repo = "Codeberg.org/Octo/Docs".parse().unwrap();
        assert_eq!(repo.to_string(), "codeberg.org/octo/docs");
        assert_eq!(repo.to_string().parse::<Repo>().unwrap(), repo);
        assert_eq!(Repo::at("https://codeberg.org/Octo/Docs.git/").unwrap(), repo);
        assert_eq!(serde_json::to_string(&repo).unwrap(), "\"codeberg.org/octo/docs\"");
        assert!(Repo::at("file:///srv/repo").is_err());
        assert!(Repo::at("https://host:8443/a/b").is_err(), "a port is no part of a forge repository");
        assert!(Repo::at("https://user@host/a/b").is_err());
        assert!("nohost".parse::<Repo>().is_err());
    }

    #[test]
    fn a_url_keeps_a_slash_inside_its_segment() {
        let url = Host::GITLAB.url(&["api", "v4", "projects", "group/sub/name"]).unwrap();
        assert_eq!(url.as_str(), "https://gitlab.com/api/v4/projects/group%2Fsub%2Fname");
    }
}
