//! What the operator decides about signing in.

use std::fmt;
use std::str::FromStr;

use jiff::Timestamp;

use super::Ttl;

/// How long a session lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expiry {
    /// Ends `idle` after it was last used, and never later than `limit` after it began.
    Sliding {
        /// How long a session may sit unused.
        idle: Ttl,
        /// The most a session lasts, however often it is used.
        limit: Ttl,
    },
    /// Ends `ttl` after it began, however often it is used.
    Absolute {
        /// How long it lasts.
        ttl: Ttl,
    },
}

impl fmt::Display for Expiry {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sliding { idle, limit } => write!(out, "sliding, {idle} idle, {limit} at most"),
            Self::Absolute { ttl } => write!(out, "absolute, {ttl}"),
        }
    }
}

/// A session's two end times: when it ends now, and the latest it can ever end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ends {
    /// When it ends if it is not used again.
    pub expires: Timestamp,
    /// The latest it can end.
    pub limit: Timestamp,
}

impl Expiry {
    /// The end times of a session that begins at `now`.
    pub fn begin(self, now: Timestamp) -> Result<Ends, jiff::Error> {
        match self {
            Self::Sliding { idle, limit } => {
                let limit = limit.after(now)?;
                Ok(Ends { expires: idle.after(now)?.min(limit), limit })
            }
            Self::Absolute { ttl } => {
                let end = ttl.after(now)?;
                Ok(Ends { expires: end, limit: end })
            }
        }
    }

    /// Where a session used at `now` now ends, when that is later than `held`.
    pub fn slide(self, now: Timestamp, ends: Ends) -> Result<Option<Timestamp>, jiff::Error> {
        match self {
            Self::Sliding { idle, .. } => {
                let next = idle.after(now)?.min(ends.limit);
                Ok((next > ends.expires).then_some(next))
            }
            Self::Absolute { .. } => Ok(None),
        }
    }
}

/// Who may make an account by signing in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Signup {
    /// Only the first account: while nobody has signed in and no first-run link is outstanding,
    /// an address may make the account that owns the instance. Nobody after.
    #[default]
    First,
    /// Any address that proves it can receive mail makes an account.
    Open,
    /// Nobody makes an account by signing in; existing accounts sign in.
    Closed,
}

impl Signup {
    /// The names a flag or a file uses.
    pub const NAMES: [(&'static str, Self); 3] =
        [("first", Self::First), ("open", Self::Open), ("closed", Self::Closed)];
}

/// A name that is no sign-up policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownSignup {
    /// What was written.
    pub written: String,
}

impl fmt::Display for UnknownSignup {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:?} is no sign-up policy: first, open or closed", self.written)
    }
}

impl std::error::Error for UnknownSignup {}

impl FromStr for Signup {
    type Err = UnknownSignup;

    fn from_str(text: &str) -> Result<Self, UnknownSignup> {
        Self::NAMES
            .iter()
            .find(|(name, _)| *name == text)
            .map(|(_, signup)| *signup)
            .ok_or_else(|| UnknownSignup { written: text.chars().take(40).collect() })
    }
}

impl fmt::Display for Signup {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(Self::NAMES.iter().find(|(_, found)| found == self).map_or("first", |(name, _)| name))
    }
}

/// Every setting of signing in. Each has a default and each is the operator's to change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// How long a sign-in link works. Default 15 minutes.
    pub login: Ttl,
    /// How long the first-run link works. Default 24 hours.
    pub admin: Ttl,
    /// How a session ends. Default: unused for 7 days, and 30 days at most.
    pub session: Expiry,
    /// Who may make an account. Default: the first.
    pub signup: Signup,
}

impl Settings {
    /// How long a sliding session may sit unused, by default.
    pub const IDLE: Ttl = Ttl::secs(7 * 86_400);
    /// The longest a session lasts, by default.
    pub const LIMIT: Ttl = Ttl::secs(30 * 86_400);
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            login: Ttl::secs(15 * 60),
            admin: Ttl::secs(24 * 3600),
            session: Expiry::Sliding { idle: Self::IDLE, limit: Self::LIMIT },
            signup: Signup::First,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_second(1_800_000_000 + seconds).unwrap()
    }

    #[test]
    fn a_sliding_session_moves_to_its_limit_and_no_further() {
        let expiry = Settings::default().session;
        let ends = expiry.begin(at(0)).unwrap();
        assert_eq!(ends.expires, at(7 * 86_400));
        assert_eq!(expiry.slide(at(86_400), ends).unwrap(), Some(at(8 * 86_400)));
        assert_eq!(expiry.slide(at(29 * 86_400), ends).unwrap(), Some(at(30 * 86_400)));
        let late = Ends { expires: at(30 * 86_400), limit: at(30 * 86_400) };
        assert_eq!(expiry.slide(at(29 * 86_400), late).unwrap(), None);
    }

    #[test]
    fn an_absolute_session_never_moves() {
        let expiry = Expiry::Absolute { ttl: "1h".parse().unwrap() };
        let ends = expiry.begin(at(0)).unwrap();
        assert_eq!(ends, Ends { expires: at(3600), limit: at(3600) });
        assert_eq!(expiry.slide(at(60), ends).unwrap(), None);
    }

    #[test]
    fn signup_policies_read_by_name() {
        assert_eq!("open".parse::<Signup>().unwrap(), Signup::Open);
        assert!("anyone".parse::<Signup>().is_err());
        assert_eq!(Signup::default().to_string(), "first");
    }
}
