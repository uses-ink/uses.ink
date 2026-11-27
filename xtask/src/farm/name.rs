//! Words that reach a command line or a path, validated once: a host name, a path segment, an
//! ssh destination. A value of these types cannot hold a space, a quote, a slash, a leading
//! dash or anything else a shell or a path would read differently from how it was written.

use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

/// What a [`Word`] names, and which characters it may hold.
pub(crate) trait Kind {
    /// What it is called in a message.
    const WHAT: &'static str;
    /// The longest it may be, in characters.
    const MOST: usize = 64;

    /// Whether `found` may appear after the first character. The first is always a letter or a digit.
    fn allows(found: char) -> bool;
}

/// The name of a host in the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Host;

/// One component of a path under a host's directory: a repo or a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Part;

/// An ssh destination: a `user@host` or an alias of the user's ssh config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Dest;

impl Kind for Host {
    const WHAT: &'static str = "host name";
    const MOST: usize = 32;

    fn allows(found: char) -> bool {
        found.is_ascii_lowercase() || found.is_ascii_digit() || matches!(found, '-' | '_')
    }
}

impl Kind for Part {
    const WHAT: &'static str = "path segment";

    fn allows(found: char) -> bool {
        found.is_ascii_alphanumeric() || matches!(found, '-' | '_' | '.')
    }
}

impl Kind for Dest {
    const WHAT: &'static str = "ssh destination";
    const MOST: usize = 128;

    fn allows(found: char) -> bool {
        found.is_ascii_alphanumeric() || matches!(found, '-' | '_' | '.' | '@')
    }
}

/// A checked word of kind `K`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Word<K>(String, PhantomData<K>);

/// A host's name.
pub(crate) type HostName = Word<Host>;
/// A repo or workspace name in a path.
pub(crate) type Segment = Word<Part>;
/// Where `ssh` connects.
pub(crate) type Destination = Word<Dest>;

impl<K> Word<K> {
    /// The word.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Word<Host> {
    /// The host that is this machine, `local`.
    pub(crate) fn local() -> Self {
        Self("local".to_owned(), PhantomData)
    }
}

impl<K: Kind> FromStr for Word<K> {
    type Err = Fault;

    fn from_str(given: &str) -> Result<Self, Fault> {
        let fault = |flaw| Fault { what: K::WHAT, given: given.to_owned(), flaw };
        let mut chars = given.chars();
        match chars.next() {
            None => return Err(fault(Flaw::Empty)),
            Some(first) if !(first.is_ascii_alphanumeric() && K::allows(first)) => return Err(fault(Flaw::Start { found: first })),
            Some(_) => {}
        }
        if let Some(found) = chars.find(|found| !K::allows(*found)) {
            return Err(fault(Flaw::Char { found }));
        }
        if given.chars().count() > K::MOST {
            return Err(fault(Flaw::Long { most: K::MOST }));
        }
        Ok(Self(given.to_owned(), PhantomData))
    }
}

impl<K: Kind> TryFrom<String> for Word<K> {
    type Error = Fault;

    fn try_from(given: String) -> Result<Self, Fault> {
        given.parse()
    }
}

impl<'de, K: Kind> serde::Deserialize<'de> for Word<K> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let given = String::deserialize(deserializer)?;
        given.parse().map_err(serde::de::Error::custom)
    }
}

impl<K> fmt::Display for Word<K> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// A word that is not allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fault {
    what: &'static str,
    given: String,
    flaw: Flaw,
}

/// What is wrong with a word.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Flaw {
    Empty,
    Start { found: char },
    Char { found: char },
    Long { most: usize },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { what, given, flaw } = self;
        match flaw {
            Flaw::Empty => write!(out, "a {what} is empty"),
            Flaw::Start { found } => write!(out, "{what} `{given}` starts with {found:?}, not a letter or a digit"),
            Flaw::Char { found } => write!(out, "{what} `{given}` holds {found:?}, which is not allowed in it"),
            Flaw::Long { most } => write!(out, "{what} `{given}` is longer than {most} characters"),
        }
    }
}

impl std::error::Error for Fault {}

/// The id of a job: the host it runs on and a token, `host.token`. A host name holds no dot, so
/// the first dot ends the host.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct JobId {
    pub(crate) host: HostName,
    token: String,
}

impl JobId {
    /// A new id for a job on `host`; `salt` tells jobs started in the same millisecond apart.
    pub(crate) fn new(host: HostName, millis: u128, salt: u32) -> Self {
        Self { host, token: format!("{millis:x}{salt:x}") }
    }

    /// The directory name of the job on its host.
    pub(crate) fn dir(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}.{}", self.host, self.token)
    }
}

impl FromStr for JobId {
    type Err = JobFault;

    fn from_str(given: &str) -> Result<Self, JobFault> {
        let (host, token) = given.split_once('.').ok_or_else(|| JobFault::Shape { given: given.to_owned() })?;
        if token.is_empty() || !token.chars().all(|found| found.is_ascii_hexdigit()) {
            return Err(JobFault::Token { given: given.to_owned() });
        }
        Ok(Self { host: host.parse().map_err(JobFault::Host)?, token: token.to_owned() })
    }
}

/// A job id that is not one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobFault {
    Shape { given: String },
    Token { given: String },
    Host(Fault),
}

impl fmt::Display for JobFault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape { given } => write!(out, "`{given}` is not a job id; they look like `host.1a2b3c`"),
            Self::Token { given } => write!(out, "`{given}` has a token that is not hexadecimal"),
            Self::Host(_) => write!(out, "the host part of the job id is not a host name"),
        }
    }
}

impl std::error::Error for JobFault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Host(fault) => Some(fault),
            Self::Shape { .. } | Self::Token { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_hold_nothing_a_shell_reads() {
        for good in ["petal", "am3", "a-b_c"] {
            assert!(good.parse::<HostName>().is_ok(), "{good}");
        }
        for bad in ["", "-x", "a b", "a;b", "a/b", "A", "a'b", "$(x)", "a\nb", ".hidden", "é"] {
            assert!(bad.parse::<HostName>().is_err(), "{bad:?}");
        }
        for bad in ["..", ".", "a/../b", "-rf", "a b", "a:b"] {
            assert!(bad.parse::<Segment>().is_err(), "{bad:?}");
            assert!(bad.parse::<Destination>().is_err(), "{bad:?}");
        }
        assert!("uses.ink".parse::<Segment>().is_ok());
        assert!("me@build-box.lan".parse::<Destination>().is_ok());
        assert!("a".repeat(33).parse::<HostName>().is_err());
    }

    #[test]
    fn job_ids_round_trip() {
        let id = JobId::new("petal".parse().unwrap(), 0x18c3_f9a2, 7);
        assert_eq!(id.to_string(), "petal.18c3f9a27");
        assert_eq!(id.to_string().parse::<JobId>().unwrap(), id);
        for bad in ["petal", "petal.", "petal.xyz", ".abc", "Pet.abc", "a b.abc"] {
            assert!(bad.parse::<JobId>().is_err(), "{bad:?}");
        }
    }
}
