//! The hosts, from a file outside the repo: the repo is public and the homelab's names are not
//! part of it. `$INK_FARM_CONFIG`, else `$XDG_CONFIG_HOME/uses-ink/farm.toml`, else
//! `~/.config/uses-ink/farm.toml`. Without a file the farm is this machine alone.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::Error;
use crate::host::{Arch, Host, HostDir, Via};
use crate::name::HostName;

/// The variables the config's location depends on.
#[derive(Debug, Clone, Default)]
pub(crate) struct Environment {
    /// `INK_FARM_CONFIG`.
    pub(crate) file: Option<OsString>,
    /// `XDG_CONFIG_HOME`.
    pub(crate) xdg: Option<OsString>,
    /// `HOME`.
    pub(crate) home: Option<OsString>,
}

impl Environment {
    /// Reads the process's variables.
    pub(crate) fn current() -> Self {
        Self {
            file: std::env::var_os("INK_FARM_CONFIG"),
            xdg: std::env::var_os("XDG_CONFIG_HOME"),
            home: std::env::var_os("HOME"),
        }
    }

    /// Where the config is, and whether the user named it: a named file that is missing is an
    /// error, the default one that is missing means no farm.
    pub(crate) fn locate(&self) -> Option<(PathBuf, bool)> {
        let tail = Path::new("uses-ink").join("farm.toml");
        match (&self.file, &self.xdg, &self.home) {
            (Some(file), ..) if !file.is_empty() => Some((PathBuf::from(file), true)),
            (_, Some(xdg), _) if !xdg.is_empty() => Some((Path::new(xdg).join(tail), false)),
            (.., Some(home)) if !home.is_empty() => Some((Path::new(home).join(".config").join(tail), false)),
            _ => None,
        }
    }
}

/// Every host, in the order the file lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) hosts: Vec<Host>,
}

impl Config {
    /// Reads the config the environment points at.
    pub(crate) fn load(env: &Environment) -> Result<Self, Error> {
        let Some((path, named)) = env.locate() else {
            return Ok(Self::local());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).map_err(|fault| Error::Config { path, fault }),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound && !named => Ok(Self::local()),
            Err(source) => Err(Error::Config { path, fault: Fault::Read(source) }),
        }
    }

    /// The farm of this machine alone.
    pub(crate) fn local() -> Self {
        Self { hosts: vec![Host::local()] }
    }

    /// Reads a config file's text.
    pub(crate) fn parse(text: &str) -> Result<Self, Fault> {
        let raw: Raw = toml::from_str(text).map_err(Fault::Toml)?;
        let mut seen = HashSet::new();
        let mut hosts = Vec::new();
        for entry in raw.host {
            if !seen.insert(entry.name.clone()) {
                return Err(Fault::Twice { name: entry.name });
            }
            hosts.push(entry.into());
        }
        if hosts.is_empty() {
            return Err(Fault::Empty);
        }
        Ok(Self { hosts })
    }
}

/// The file as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    host: Vec<Entry>,
}

/// One `[[host]]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: HostName,
    ssh: Ssh,
    slots: Option<NonZeroU32>,
    arch: Option<ArchText>,
    golden: Option<bool>,
    #[serde(default)]
    mold: bool,
    dir: Option<DirText>,
}

/// The `ssh` key: `local` or an ssh destination.
#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
struct Ssh(Via);

impl TryFrom<String> for Ssh {
    type Error = crate::name::Fault;

    fn try_from(given: String) -> Result<Self, Self::Error> {
        given.parse().map(Self)
    }
}

/// The `arch` key.
#[derive(Debug, Deserialize)]
#[serde(from = "String")]
struct ArchText(Arch);

impl From<String> for ArchText {
    fn from(given: String) -> Self {
        Self(given.parse().unwrap_or_else(|never: std::convert::Infallible| match never {}))
    }
}

/// The `dir` key.
#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
struct DirText(HostDir);

impl TryFrom<String> for DirText {
    type Error = crate::host::DirFault;

    fn try_from(given: String) -> Result<Self, Self::Error> {
        given.parse().map(Self)
    }
}

impl From<Entry> for Host {
    fn from(entry: Entry) -> Self {
        Self {
            name: entry.name,
            via: entry.ssh.0,
            slots: entry.slots.map(NonZeroU32::get),
            arch: entry.arch.map(|arch| arch.0),
            golden: entry.golden,
            mold: entry.mold,
            dir: entry.dir.map_or_else(HostDir::standard, |dir| dir.0),
        }
    }
}

/// A config that cannot be used.
#[derive(Debug)]
pub(crate) enum Fault {
    Read(std::io::Error),
    Toml(toml::de::Error),
    /// Two hosts with one name.
    Twice { name: HostName },
    /// A file with no `[[host]]`.
    Empty,
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(_) => write!(out, "cannot be read"),
            Self::Toml(_) => write!(out, "is not the TOML expected"),
            Self::Twice { name } => write!(out, "names the host `{name}` twice"),
            Self::Empty => write!(out, "has no `[[host]]`; delete it to use this machine alone"),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(source) => Some(source),
            Self::Toml(source) => Some(source),
            Self::Twice { .. } | Self::Empty => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"
[[host]]
name = "petal"
ssh = "me@petal.lan"
slots = 1
mold = true

[[host]]
name = "am3"
ssh = "am3"
arch = "aarch64"
dir = "/srv/farm"

[[host]]
name = "here"
ssh = "local"
golden = true
"#;

    #[test]
    fn a_file_reads_into_hosts_with_defaults() {
        let config = Config::parse(FILE).unwrap();
        let names: Vec<&str> = config.hosts.iter().map(|host| host.name.as_str()).collect();
        assert_eq!(names, ["petal", "am3", "here"]);
        let petal = config.hosts.first().unwrap();
        assert_eq!((petal.slots, petal.golden, petal.mold), (Some(1), None, true));
        assert_eq!(petal.dir, HostDir::standard());
        let am3 = config.hosts.get(1).unwrap();
        assert_eq!(am3.arch, Some(Arch::Aarch64));
        assert_eq!(am3.dir.as_str(), "/srv/farm");
        assert_eq!(config.hosts.get(2).unwrap().via, Via::Local);
    }

    #[test]
    fn a_bad_file_says_why() {
        let table = [
            ("", "no `[[host]]`"),
            ("[[host]]\nname = \"a\"\nssh = \"x\"\n[[host]]\nname = \"a\"\nssh = \"y\"\n", "twice"),
            ("[[host]]\nname = \"a b\"\nssh = \"x\"\n", "host name"),
            ("[[host]]\nname = \"a\"\nssh = \"-oProxyCommand=x\"\n", "ssh destination"),
            ("[[host]]\nname = \"a\"\nssh = \"x\"\nslots = 0\n", "slots"),
            ("[[host]]\nname = \"a\"\nssh = \"x\"\ndir = \"../x\"\n", "`..`"),
            ("[[host]]\nname = \"a\"\nssh = \"x\"\nwat = 1\n", "wat"),
        ];
        for (text, needle) in table {
            let fault = Config::parse(text).unwrap_err();
            let mut shown = fault.to_string();
            let mut cause = std::error::Error::source(&fault);
            while let Some(error) = cause {
                shown.push_str(&error.to_string());
                cause = error.source();
            }
            assert!(shown.contains(needle), "{shown}");
        }
    }

    #[test]
    fn the_location_follows_the_variables() {
        let env = |file: Option<&str>, xdg: Option<&str>, home: Option<&str>| Environment {
            file: file.map(Into::into),
            xdg: xdg.map(Into::into),
            home: home.map(Into::into),
        };
        let table = [
            (env(Some("/x/f.toml"), Some("/c"), Some("/h")), Some(("/x/f.toml", true))),
            (env(None, Some("/c"), Some("/h")), Some(("/c/uses-ink/farm.toml", false))),
            (env(None, None, Some("/h")), Some(("/h/.config/uses-ink/farm.toml", false))),
            (env(Some(""), Some(""), Some("/h")), Some(("/h/.config/uses-ink/farm.toml", false))),
            (env(None, None, None), None),
        ];
        for (environment, want) in table {
            let got = environment.locate();
            assert_eq!(got.as_ref().map(|(path, named)| (path.to_str().unwrap(), *named)), want);
        }
    }

    #[test]
    fn no_file_is_this_machine_alone() {
        let env = Environment { file: None, xdg: Some("/nonexistent-farm-config".into()), home: None };
        assert_eq!(Config::load(&env).unwrap(), Config::local());
        let named = Environment { file: Some("/nonexistent-farm-config/f.toml".into()), ..Environment::default() };
        assert!(Config::load(&named).is_err());
    }
}
