//! Enrolling a device or deploy key (`docs/auth.md` section 6): the device flow of RFC 8628,
//! adapted. The key pair is the secret, so no device code and no access token exist; the person
//! compares a fingerprint and approves.

use std::fmt;
use std::str::FromStr;

use db::Row;
use jiff::Timestamp;
use sig::{Fingerprint, KeyId, KeyType, PublicKey};

use super::key::{Kind, Label};
use super::right::Rights;
use crate::Unreadable;
use crate::fields::Fields;
use crate::id::{Enrolment as EnrolmentKind, Id, Site as SiteKind};
use crate::signin::Ttl;
use crate::table;

/// The short code a person types on the approving screen: 8 letters from a 20-letter alphabet
/// without vowels or lookalikes, about 34 bits, shown `HRKM-QWPD`. It is not secret-grade and is
/// stored as text: a leaked pending code enrols nothing, because approval needs a person who is
/// signed in and the key is not the guesser's.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct UserCode(String);

/// Text that is no user code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserCodeError {
    /// What was written, cut to a length that fits a message.
    pub written: String,
}

impl fmt::Display for UserCodeError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is not a code of {} letters like HRKM-QWPD", self.written, UserCode::LETTERS)
    }
}

impl std::error::Error for UserCodeError {}

impl UserCode {
    /// The alphabet: the consonants without `L`.
    pub const ALPHABET: &'static [u8; 20] = b"BCDFGHJKMNPQRSTVWXYZ";

    /// How many letters.
    pub const LETTERS: usize = 8;

    /// A code from one draw of 16 bytes: two bytes make a letter.
    pub fn drawn(bytes: [u8; 16]) -> Self {
        let letters = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let value = pair.iter().fold(0_usize, |all, byte| (all << 8) | usize::from(*byte));
                Self::ALPHABET.get(value % Self::ALPHABET.len()).map_or('B', |letter| char::from(*letter))
            })
            .collect();
        Self(letters)
    }

    /// The letters, without the dash.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for UserCode {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "UserCode({self})")
    }
}

impl fmt::Display for UserCode {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (head, tail) = self.0.split_at(4.min(self.0.len()));
        write!(out, "{head}-{tail}")
    }
}

impl FromStr for UserCode {
    type Err = UserCodeError;

    /// Upper or lower case, with or without the dash.
    fn from_str(text: &str) -> Result<Self, UserCodeError> {
        let bare: String = text.chars().filter(|c| *c != '-').map(|c| c.to_ascii_uppercase()).collect();
        let valid = bare.len() == Self::LETTERS && bare.bytes().all(|byte| Self::ALPHABET.contains(&byte));
        if valid { Ok(Self(bare)) } else { Err(UserCodeError { written: text.chars().take(24).collect() }) }
    }
}

/// Where an enrolment stands. `Expired` is not stored: it is a pending one past its time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum State {
    /// Waiting for a person.
    Pending,
    /// A person approved it, and the key is live.
    Approved,
    /// A person turned it down.
    Denied,
}

impl State {
    /// The names a column stores.
    pub const NAMES: [(&'static str, Self); 3] =
        [("pending", Self::Pending), ("approved", Self::Approved), ("denied", Self::Denied)];

    /// The name it is stored as.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }
}

/// What a poll answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Poll {
    /// Not yet.
    Pending,
    /// Approved: the key is live until `expires`.
    Approved {
        /// When the key ends.
        expires: Timestamp,
    },
    /// Turned down.
    Denied,
    /// Ran out of time.
    Expired,
}

/// A key waiting for a person to approve it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Enrolment {
    /// Its id.
    pub id: Id<EnrolmentKind>,
    /// The code a person types.
    pub code: UserCode,
    /// The key's type.
    pub key_type: KeyType,
    /// The public key that would be enrolled.
    pub public: PublicKey,
    /// What the requester calls it. Text from the requester, not fact.
    pub label: Label,
    /// A device key, or a deploy key.
    pub purpose: Kind,
    /// The site a deploy key is for.
    pub site: Option<Id<SiteKind>>,
    /// The rights asked for; empty asks for the kind's defaults.
    pub rights: Rights,
    /// How long the key lives once approved; the kind's default when none was asked.
    pub lifetime: Ttl,
    /// Where it stands.
    pub state: State,
    /// When it was asked.
    pub created: Timestamp,
    /// When the ask runs out.
    pub expires: Timestamp,
}

impl Enrolment {
    /// The columns a read selects.
    pub(crate) const COLUMNS: [table::Enrolment; 12] = [
        table::Enrolment::Id,
        table::Enrolment::UserCode,
        table::Enrolment::KeyType,
        table::Enrolment::PublicKey,
        table::Enrolment::Label,
        table::Enrolment::Purpose,
        table::Enrolment::SiteId,
        table::Enrolment::Rights,
        table::Enrolment::Lifetime,
        table::Enrolment::State,
        table::Enrolment::CreatedAt,
        table::Enrolment::ExpiresAt,
    ];

    /// The id of the key it would enrol.
    pub fn key(&self) -> KeyId {
        self.public.id(self.key_type)
    }

    /// What a poll answers at `now`.
    pub fn poll(&self, approved: Option<Timestamp>, now: Timestamp) -> Poll {
        match (self.state, approved) {
            (State::Approved, Some(expires)) => Poll::Approved { expires },
            (State::Denied, _) => Poll::Denied,
            (State::Pending, _) if self.expires <= now => Poll::Expired,
            (State::Approved, None) | (State::Pending, _) => Poll::Pending,
        }
    }

    /// The fingerprint a person compares.
    pub fn fingerprint(&self) -> Fingerprint {
        self.key().fingerprint()
    }
}

impl TryFrom<&Row> for Enrolment {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let bytes: Vec<u8> = row.get(table::Enrolment::PublicKey)?;
        let public = <[u8; 32]>::try_from(bytes.as_slice())
            .map(PublicKey::new)
            .map_err(|_| Unreadable::Digest { column: "public_key" })?;
        let seconds = fields.count(table::Enrolment::Lifetime)?;
        Ok(Self {
            id: fields.id(table::Enrolment::Id)?,
            code: fields
                .text(table::Enrolment::UserCode)?
                .parse()
                .map_err(|_| Unreadable::Text { column: "user_code" })?,
            key_type: fields.name(table::Enrolment::KeyType, &KeyType::NAMES)?,
            public,
            label: Label::parse(&fields.text(table::Enrolment::Label)?)
                .map_err(|_| Unreadable::Text { column: "label" })?,
            purpose: fields.name(table::Enrolment::Purpose, &Kind::NAMES)?,
            site: fields.maybe_id(table::Enrolment::SiteId)?,
            rights: Rights::read(&fields.text(table::Enrolment::Rights)?)
                .map_err(|source| Unreadable::Name { column: "rights", found: source.written })?,
            lifetime: Ttl::secs(seconds),
            state: fields.name(table::Enrolment::State, &State::NAMES)?,
            created: fields.time(table::Enrolment::CreatedAt)?,
            expires: fields.time(table::Enrolment::ExpiresAt)?,
        })
    }
}

/// An enrolment to make: everything the requester sent and everything the caller drew.
#[derive(Clone, Debug)]
pub struct NewEnrolment {
    /// Its id.
    pub id: Id<EnrolmentKind>,
    /// The code, from a draw.
    pub code: UserCode,
    /// The key's type.
    pub key_type: KeyType,
    /// The public key.
    pub public: PublicKey,
    /// The label.
    pub label: Label,
    /// A device key or a deploy key.
    pub purpose: Kind,
    /// The site, for a deploy key.
    pub site: Option<Id<SiteKind>>,
    /// The rights asked for; empty for the defaults.
    pub rights: Rights,
    /// How long the key should live; `None` for the kind's default.
    pub lifetime: Option<Ttl>,
    /// The time now.
    pub at: Timestamp,
}

/// What a person sees before approving: the key's fingerprint, its label as text from the
/// requester, what it would be allowed, and when it asked.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Seen {
    /// The fingerprint to compare with the terminal.
    pub fingerprint: Fingerprint,
    /// The label.
    pub label: Label,
    /// The key's type.
    pub key_type: KeyType,
    /// A device key or a deploy key.
    pub purpose: Kind,
    /// The site, for a deploy key.
    pub site: Option<Id<SiteKind>>,
    /// The rights it would have.
    pub rights: Rights,
    /// How long it would live.
    pub lifetime: Ttl,
    /// When it asked.
    pub asked: Timestamp,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_user_code_is_eight_letters_and_reads_any_way_it_is_typed() {
        let code = UserCode::drawn([0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8]);
        assert_eq!(code.as_str().len(), 8);
        assert_eq!(code.to_string().len(), 9);
        assert_eq!(code.to_string().parse::<UserCode>(), Ok(code.clone()));
        assert_eq!(code.as_str().to_lowercase().parse::<UserCode>(), Ok(code));
        assert!("HRKM-LWPD".parse::<UserCode>().is_err());
        assert!("HRK".parse::<UserCode>().is_err());
        assert!("AEIOUAEI".parse::<UserCode>().is_err());
    }
}
