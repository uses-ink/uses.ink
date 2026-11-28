//! The Forgejo instances this server talks to.

use std::fmt;

use serde::Deserialize;

use authority_forge::Host;

/// One Forgejo instance.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    /// What people call it.
    pub name: String,
    /// Where it is.
    pub host: Host,
    /// Whether the project's OAuth application is registered there.
    #[serde(default)]
    pub preregistered: bool,
    /// Whether it is the one a person gets when they name none.
    #[serde(default)]
    pub primary: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    instance: Vec<Instance>,
}

/// A list of instances that could not be read.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unreadable {
    /// The TOML is wrong, or a host is no host.
    Toml(toml::de::Error),
    /// The list names this many primaries, not one.
    Primary {
        /// How many.
        found: usize,
    },
    /// A host is listed twice.
    Twice {
        /// The host.
        host: Host,
    },
}

impl Unreadable {
    /// The code of the catalog that words this.
    pub const fn code(&self) -> &'static str {
        "forge.instances.invalid"
    }
}

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(_) => out.write_str("the list of Forgejo instances is not valid"),
            Self::Primary { found } => write!(out, "the list of Forgejo instances has {found} primaries, not one"),
            Self::Twice { host } => write!(out, "the list of Forgejo instances has {host} twice"),
        }
    }
}

impl std::error::Error for Unreadable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Toml(source) => Some(source),
            Self::Primary { .. } | Self::Twice { .. } => None,
        }
    }
}

/// The instances, with exactly one primary and no host twice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instances {
    list: Vec<Instance>,
    primary: usize,
}

impl Instances {
    /// The file that ships: Codeberg.
    pub const BUNDLED: &'static str = include_str!("../instances.toml");

    /// The bundled list. Call it once at start and hold the result.
    pub fn bundled() -> Result<Self, Unreadable> {
        Self::read(Self::BUNDLED)
    }

    /// The list `text` spells, for an operator that adds an instance.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let file: File = toml::from_str(text).map_err(Unreadable::Toml)?;
        let primaries: Vec<usize> =
            file.instance.iter().enumerate().filter(|(_, instance)| instance.primary).map(|(at, _)| at).collect();
        let [primary] = primaries[..] else { return Err(Unreadable::Primary { found: primaries.len() }) };
        for (at, instance) in file.instance.iter().enumerate() {
            if file.instance.iter().skip(at + 1).any(|other| other.host == instance.host) {
                return Err(Unreadable::Twice { host: instance.host.clone() });
            }
        }
        Ok(Self { list: file.instance, primary })
    }

    /// The instance at `host`, when it is listed.
    pub fn find(&self, host: &Host) -> Option<&Instance> {
        self.list.iter().find(|instance| &instance.host == host)
    }

    /// The instance a person gets when they name none.
    pub fn primary(&self) -> Option<&Instance> {
        self.list.get(self.primary)
    }

    /// Every instance, in the order listed.
    pub fn all(&self) -> &[Instance] {
        &self.list
    }
}
