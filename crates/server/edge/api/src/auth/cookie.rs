//! The session cookie: what is set, and how it is read back.
//!
//! The attributes are fixed by what a session needs and one switch, `secure`:
//!
//! - `HttpOnly`: a script on the page cannot read the session.
//! - `Secure`, unless the instance is local (plain `http` on a loopback name): the cookie is sent
//!   over TLS only.
//! - `SameSite=Lax`: the browser does not send it on a cross-site `POST` or `DELETE`, which is the
//!   CSRF defence of the cookie. A top-level link from a mail to the instance still carries it.
//! - `Path`, from the [`Jar`] (default `/`), and `Max-Age`, set to the session's limit.
//! - No `Domain`: the cookie belongs to the host that set it alone.
//!
//! The name is configurable. The default is `__Host-ink-session` when `Secure` (the browser then
//! also insists on `Secure`, `Path=/` and no `Domain`) and `ink-session` otherwise.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use control::signin::Proof;

/// A cookie name: 1 to 64 characters of the `token` class of RFC 6265.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name(String);

/// Text that is no cookie name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameError {
    /// Empty, or more than 64 bytes.
    Length {
        /// How long.
        found: usize,
    },
    /// A character that a name may not hold, at this byte offset.
    Character {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for NameError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a cookie name is 1 to 64 bytes, not {found}"),
            Self::Character { at } => write!(out, "byte {at} of the cookie name is not allowed in a name"),
        }
    }
}

impl std::error::Error for NameError {}

impl FromStr for Name {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        if text.is_empty() || text.len() > 64 {
            return Err(NameError::Length { found: text.len() });
        }
        let allowed = |byte: u8| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte);
        match text.bytes().position(|byte| !allowed(byte)) {
            Some(at) => Err(NameError::Character { at }),
            None => Ok(Self(text.to_owned())),
        }
    }
}

impl fmt::Display for Name {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// A cookie setting that cannot work.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum JarError {
    /// The path does not start with `/` or holds a character a path may not.
    Path {
        /// The path.
        path: String,
    },
    /// A `__Host-` name needs `Secure` and the path `/`.
    Host,
}

impl fmt::Display for JarError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path { path } => {
                write!(out, "{path:?} is not a cookie path: start with / and use no space, `;` or `,`")
            }
            Self::Host => out.write_str("a `__Host-` cookie needs Secure and the path /"),
        }
    }
}

impl std::error::Error for JarError {}

/// How the session cookie is named, scoped and secured. Every field is the operator's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jar {
    name: Name,
    path: String,
    secure: bool,
}

impl Jar {
    /// A jar for a name (the default for `secure` when none is given), a path and whether the
    /// cookie is `Secure`.
    pub fn new(name: Option<Name>, path: &str, secure: bool) -> Result<Self, JarError> {
        let graphic = path.bytes().all(|byte| byte.is_ascii_graphic() && byte != b';' && byte != b',');
        if !path.starts_with('/') || !graphic {
            return Err(JarError::Path { path: path.chars().take(80).collect() });
        }
        let default = if secure { "__Host-ink-session" } else { "ink-session" };
        let name = name.unwrap_or(Name(default.to_owned()));
        if name.0.starts_with("__Host-") && (!secure || path != "/") {
            return Err(JarError::Host);
        }
        Ok(Self { name, path: path.to_owned(), secure })
    }

    /// The cookie's name.
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// The cookie that holds `value` for `age`.
    pub fn set(&self, value: &Proof, age: Duration) -> Cookie {
        Cookie {
            name: self.name.clone(),
            value: value.reveal().to_owned(),
            path: self.path.clone(),
            secure: self.secure,
            age,
        }
    }

    /// The cookie that makes the browser forget the session.
    pub fn clear(&self) -> Cookie {
        Cookie {
            name: self.name.clone(),
            value: String::new(),
            path: self.path.clone(),
            secure: self.secure,
            age: Duration::ZERO,
        }
    }

    /// The session secret in a `Cookie` header, if this jar's cookie is there and is shaped like
    /// one.
    pub fn read(&self, header: &[u8]) -> Option<Proof> {
        let text = std::str::from_utf8(header).ok()?;
        text.split(';')
            .filter_map(|pair| pair.trim().split_once('='))
            .filter(|(name, _)| *name == self.name.0)
            .find_map(|(_, value)| value.parse().ok())
    }
}

/// A `Set-Cookie`. Its value is a secret: it does not print.
#[derive(Clone, PartialEq, Eq)]
pub struct Cookie {
    name: Name,
    value: String,
    path: String,
    secure: bool,
    age: Duration,
}

impl fmt::Debug for Cookie {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Cookie({}=<redacted>)", self.name)
    }
}

impl fmt::Display for Cookie {
    /// The `Set-Cookie` header's value.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{}={}; Path={}; Max-Age={}; HttpOnly; SameSite=Lax",
            self.name,
            self.value,
            self.path,
            self.age.as_secs()
        )?;
        if self.secure {
            out.write_str("; Secure")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn proof() -> Proof {
        Proof::from_bytes([[1; 16], [2; 16]])
    }

    #[test]
    fn a_secure_jar_sets_a_host_cookie_with_every_attribute() {
        let jar = Jar::new(None, "/", true).unwrap();
        let text = jar.set(&proof(), Duration::from_secs(60)).to_string();
        assert!(text.starts_with("__Host-ink-session=0101"), "{text}");
        for attribute in ["Path=/", "Max-Age=60", "HttpOnly", "SameSite=Lax", "Secure"] {
            assert!(text.contains(attribute), "{text}");
        }
        assert!(!text.contains("Domain"), "{text}");
    }

    #[test]
    fn a_local_jar_is_not_secure() {
        let text = Jar::new(None, "/", false).unwrap().set(&proof(), Duration::from_secs(60)).to_string();
        assert!(text.starts_with("ink-session="), "{text}");
        assert!(!text.contains("Secure"), "{text}");
    }

    #[test]
    fn a_cleared_cookie_has_no_value_and_no_age() {
        let text = Jar::new(None, "/app", false).unwrap().clear().to_string();
        assert_eq!(text, "ink-session=; Path=/app; Max-Age=0; HttpOnly; SameSite=Lax");
    }

    #[test]
    fn a_cookie_never_prints_its_value() {
        let jar = Jar::new(None, "/", true).unwrap();
        let cookie = jar.set(&proof(), Duration::from_secs(60));
        assert_eq!(format!("{cookie:?}"), "Cookie(__Host-ink-session=<redacted>)");
    }

    #[test]
    fn reads_its_own_cookie_among_others() {
        let jar = Jar::new(None, "/", false).unwrap();
        let value = proof();
        let header = format!("a=b; ink-session={}; c=d", value.reveal());
        assert_eq!(jar.read(header.as_bytes()).unwrap().reveal(), value.reveal());
        assert!(jar.read(b"ink-session=short").is_none());
        assert!(jar.read(b"other=1").is_none());
    }

    #[test]
    fn settings_that_cannot_work_are_refused() {
        assert!(matches!(Jar::new(None, "app", true), Err(JarError::Path { .. })));
        assert!(matches!(Jar::new(None, "/a;b", true), Err(JarError::Path { .. })));
        assert_eq!(Jar::new(None, "/app", true), Err(JarError::Host));
        assert_eq!("bad name".parse::<Name>(), Err(NameError::Character { at: 3 }));
        assert!("sid".parse::<Name>().is_ok());
    }
}
