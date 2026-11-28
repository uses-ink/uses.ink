//! What a TXT record says, and what a person is told to publish.

use std::fmt;

use control::Token;

use crate::Host;

/// The text of a proof record: `uses-ink-domain=<token>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record;

impl Record {
    /// What comes before the token.
    pub const PREFIX: &'static str = "uses-ink-domain=";

    /// The record text for `token`.
    pub fn write(token: &Token) -> String {
        format!("{}{token}", Self::PREFIX)
    }

    /// The token a record carries, when it is one of ours. Spaces around it and quotes a DNS
    /// client leaves on a TXT string are not part of it.
    pub fn read(text: &str) -> Option<Token> {
        text.trim().trim_matches('"').trim().strip_prefix(Self::PREFIX)?.parse().ok()
    }
}

/// What a person publishes to prove a name: a TXT record, at a host, with a value.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Challenge {
    /// Where to publish it.
    pub host: Host,
    /// What it says.
    pub value: String,
}

impl Challenge {
    /// The record that proves `claim`.
    pub fn of(claim: &control::Claim) -> Self {
        Self { host: Host::of(&claim.name), value: Record::write(&claim.token) }
    }
}

impl fmt::Display for Challenge {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} TXT \"{}\"", self.host, self.value)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_record_round_trips_and_other_text_is_no_record() {
        let token = Token::from_bytes([7; 16]);
        assert_eq!(Record::read(&Record::write(&token)), Some(token));
        assert_eq!(Record::read(&format!("  \"{}\" ", Record::write(&token))), Some(token));
        assert_eq!(Record::read("v=spf1 -all"), None);
        assert_eq!(Record::read("uses-ink-domain=short"), None);
        assert_eq!(Record::read(""), None);
    }
}
