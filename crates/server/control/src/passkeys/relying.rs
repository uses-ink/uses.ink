//! The relying party: the id credentials are bound to, and the origins that may ask for them.

use std::fmt;

/// Who passkeys are made for (`docs/auth.md` section 11): a relying party id, a name the
/// authenticator shows, and the exact origins of the dashboard. Hosted it is `app.uses.ink`;
/// a self-host or a test names its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relying {
    id: String,
    name: String,
    origins: Vec<String>,
}

/// A relying party that cannot work.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RelyingError {
    /// The id is not a lower-case host name (letters, digits, `-` and `.`).
    Id {
        /// What was given.
        id: String,
    },
    /// The id is an IP address, which WebAuthn does not take.
    Address {
        /// What was given.
        id: String,
    },
    /// No origin was given, so no page could ever ask.
    NoOrigin,
    /// The origin is not `scheme://host` with an optional port and nothing after it.
    Origin {
        /// What was given.
        origin: String,
    },
    /// An `http` origin that is not `localhost` or under it: browsers allow WebAuthn only over
    /// TLS, or on localhost.
    Insecure {
        /// What was given.
        origin: String,
    },
    /// The origin's host is neither the id nor under it, so the browser would refuse it.
    Unrelated {
        /// What was given.
        origin: String,
        /// The id.
        id: String,
    },
}

impl fmt::Display for RelyingError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id { id } => write!(out, "`{id}` is not a relying party id: use a lower-case host name"),
            Self::Address { id } => {
                write!(out, "`{id}` is an address, and WebAuthn needs a domain name or `localhost`")
            }
            Self::NoOrigin => out.write_str("a relying party needs at least one origin"),
            Self::Origin { origin } => {
                write!(out, "`{origin}` is not an origin: write `https://host` or `https://host:port`")
            }
            Self::Insecure { origin } => {
                write!(out, "`{origin}` is plain http, which WebAuthn allows only on localhost")
            }
            Self::Unrelated { origin, id } => write!(out, "the host of `{origin}` is not `{id}` or under it"),
        }
    }
}

impl std::error::Error for RelyingError {}

impl Relying {
    /// The hosted dashboard.
    pub const HOSTED: &'static str = "app.uses.ink";

    /// Whether `text` is a lower-case host name.
    fn host_name(text: &str) -> bool {
        !text.is_empty()
            && !text.starts_with('.')
            && !text.ends_with('.')
            && text.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-.".contains(&byte))
    }

    /// The relying party `id` named `name`, asked from `origins`.
    pub fn new(id: &str, name: &str, origins: &[&str]) -> Result<Self, RelyingError> {
        if !Self::host_name(id) {
            return Err(RelyingError::Id { id: id.to_owned() });
        }
        if id.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.') {
            return Err(RelyingError::Address { id: id.to_owned() });
        }
        if origins.is_empty() {
            return Err(RelyingError::NoOrigin);
        }
        for origin in origins {
            let (scheme, rest) = origin
                .split_once("://")
                .filter(|(scheme, _)| ["https", "http"].contains(scheme))
                .ok_or_else(|| RelyingError::Origin { origin: (*origin).to_owned() })?;
            let (host, port) = match rest.split_once(':') {
                Some((host, port)) => (host, Some(port)),
                None => (rest, None),
            };
            let port_ok = port.is_none_or(|port| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()));
            if !Self::host_name(host) || !port_ok {
                return Err(RelyingError::Origin { origin: (*origin).to_owned() });
            }
            let local = host == "localhost" || host.ends_with(".localhost");
            if scheme == "http" && !local {
                return Err(RelyingError::Insecure { origin: (*origin).to_owned() });
            }
            if host != id && !host.ends_with(&format!(".{id}")) {
                return Err(RelyingError::Unrelated { origin: (*origin).to_owned(), id: id.to_owned() });
            }
        }
        Ok(Self {
            id: id.to_owned(),
            name: name.to_owned(),
            origins: origins.iter().map(|origin| (*origin).to_owned()).collect(),
        })
    }

    /// The hosted dashboard's relying party.
    pub fn hosted() -> Self {
        Self {
            id: Self::HOSTED.to_owned(),
            name: "uses.ink".to_owned(),
            origins: vec![format!("https://{}", Self::HOSTED)],
        }
    }

    /// The relying party id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The name an authenticator shows.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The origins that may ask.
    pub fn origins(&self) -> &[String] {
        &self.origins
    }

    /// Whether `origin` is one of the configured origins, exactly (scheme, host and port).
    pub fn allows(&self, origin: &str) -> bool {
        self.origins.iter().any(|known| known == origin)
    }
}

impl fmt::Display for Relying {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "relying party id `{}`, origins {}", self.id, self.origins.join(", "))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn hosted_is_the_dashboard_host() {
        let hosted = Relying::hosted();
        assert_eq!(hosted.id(), "app.uses.ink");
        assert!(hosted.allows("https://app.uses.ink"));
        assert!(!hosted.allows("https://uses.ink"));
        assert_eq!(Relying::new("app.uses.ink", "uses.ink", &["https://app.uses.ink"]).unwrap(), hosted);
    }

    #[test]
    fn what_a_browser_would_refuse_is_refused_here() {
        let new = |id, origins: &[&str]| Relying::new(id, "x", origins);
        assert_eq!(new("Example.com", &["https://example.com"]), Err(RelyingError::Id { id: "Example.com".into() }));
        assert_eq!(new("127.0.0.1", &["http://127.0.0.1"]), Err(RelyingError::Address { id: "127.0.0.1".into() }));
        assert_eq!(new("example.com", &[]), Err(RelyingError::NoOrigin));
        assert_eq!(new("example.com", &["example.com"]), Err(RelyingError::Origin { origin: "example.com".into() }));
        assert_eq!(
            new("example.com", &["https://example.com/x"]),
            Err(RelyingError::Origin { origin: "https://example.com/x".into() })
        );
        assert_eq!(
            new("example.com", &["http://example.com"]),
            Err(RelyingError::Insecure { origin: "http://example.com".into() })
        );
        assert_eq!(
            new("example.com", &["https://example.org"]),
            Err(RelyingError::Unrelated { origin: "https://example.org".into(), id: "example.com".into() })
        );
        assert!(new("example.com", &["https://ink.example.com:8443"]).is_ok());
        assert!(new("localhost", &["http://localhost:8080"]).is_ok());
    }
}
