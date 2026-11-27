//! What a private repository is read with.

use std::fmt;

use control::{Control, Id, Ports, Purpose, SiteKind};
use secret::Plain;

/// A user name and a password or token. It redacts itself in `Debug`, has no `Display`, and the
/// secret is zeroed when it is dropped.
///
/// A stored git token is read as `user:password` when it holds a colon and as a password for the
/// user [`Access::USER`] when it does not, which is what token-only forges (a personal access
/// token) ask for.
pub struct Access {
    whole: Plain,
    colon: Option<usize>,
}

/// A stored secret that is no access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccessError {
    /// The secret is not UTF-8 text.
    Text,
    /// The secret is empty.
    Empty,
}

impl fmt::Display for AccessError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Text => "a git credential is text, `user:password` or a token",
            Self::Empty => "a git credential is not empty",
        })
    }
}

impl std::error::Error for AccessError {}

impl Access {
    /// The user a token with no user is sent as.
    pub const USER: &'static str = "git";

    fn text(&self) -> &str {
        // checked when it was made
        std::str::from_utf8(self.whole.expose()).unwrap_or_default()
    }

    /// What the newest git token of `site` opens to, or none when it has no token. It is
    /// opened for one use and dropped (zeroed) after it.
    pub async fn of<P: Ports + 'static>(
        control: &Control<P>,
        site: Id<SiteKind>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let stored = control.credentials(site).await?;
        let Some(token) = stored.iter().rev().find(|credential| credential.purpose == Purpose::GitToken) else {
            return Ok(None);
        };
        let plain = control.credential(token.id).await?;
        Ok(Some(Self::try_from(plain)?))
    }

    /// The user name.
    pub fn user(&self) -> &str {
        self.colon.map_or(Self::USER, |at| self.text().get(..at).unwrap_or_default())
    }

    /// The password or token. Never print it.
    pub fn password(&self) -> &str {
        match self.colon {
            Some(at) => self.text().get(at + 1..).unwrap_or_default(),
            None => self.text(),
        }
    }
}

impl TryFrom<Plain> for Access {
    type Error = AccessError;

    fn try_from(whole: Plain) -> Result<Self, AccessError> {
        let text = std::str::from_utf8(whole.expose()).map_err(|_| AccessError::Text)?;
        if text.is_empty() {
            return Err(AccessError::Empty);
        }
        let colon = text.find(':');
        Ok(Self { whole, colon })
    }
}

impl fmt::Debug for Access {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Access(..)")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn reads_a_user_and_a_password_or_a_token() {
        let both = Access::try_from(Plain::from("ann:s3cret")).unwrap();
        assert_eq!((both.user(), both.password()), ("ann", "s3cret"));
        let token = Access::try_from(Plain::from("ghp_abc")).unwrap();
        assert_eq!((token.user(), token.password()), (Access::USER, "ghp_abc"));
        assert_eq!(Access::try_from(Plain::from("")).unwrap_err(), AccessError::Empty);
        assert_eq!(Access::try_from(Plain::from(vec![0xff, 0xfe])).unwrap_err(), AccessError::Text);
    }

    #[test]
    fn never_prints_the_secret() {
        let access = Access::try_from(Plain::from("ann:s3cret")).unwrap();
        assert!(!format!("{access:?}").contains("s3cret"));
    }
}
