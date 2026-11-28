//! A device key or a deploy key an account holds.

use std::fmt;

use db::Row;
use jiff::Timestamp;
use jiff::civil::Date;
use sig::{KeyId, KeyType, PublicKey};

use super::right::{Grant, Grants, Rights, Scope};
use control_rows::fields::Fields;
use control_rows::id::{Account as AccountKind, Id, Site as SiteKind};
use control_rows::table;
use control_rows::unreadable::Unreadable;
use control_signin::Credential;

/// What a key is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Kind {
    /// The CLI on a laptop: acts for a person, 180 days by default.
    Device,
    /// A CI or deploy job: one site, 90 days by default.
    Deploy,
}

impl Kind {
    /// The names a column stores.
    pub const NAMES: [(&'static str, Self); 2] = [("device", Self::Device), ("deploy", Self::Deploy)];

    /// The name it is stored and written as.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Device => "device",
            Self::Deploy => "deploy",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// Text that is no kind of key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownKind {
    /// What was written.
    pub written: String,
}

impl fmt::Display for UnknownKind {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is no kind of key: the kinds are device and deploy", self.written)
    }
}

impl std::error::Error for UnknownKind {}

impl std::str::FromStr for Kind {
    type Err = UnknownKind;

    fn from_str(text: &str) -> Result<Self, UnknownKind> {
        Self::NAMES
            .iter()
            .find(|(name, _)| *name == text)
            .map(|(_, kind)| *kind)
            .ok_or_else(|| UnknownKind { written: text.to_owned() })
    }
}

/// Why a key stopped working.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Why {
    /// Someone with the `keys` right revoked it.
    Revoked,
    /// The key revoked itself (`ink logout`).
    LoggedOut,
}

impl Why {
    /// The names a column stores.
    pub const NAMES: [(&'static str, Self); 2] = [("revoked", Self::Revoked), ("logged-out", Self::LoggedOut)];

    /// The name it is stored and written as.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Revoked => "revoked",
            Self::LoggedOut => "logged-out",
        }
    }
}

/// A label a person gives a key: text from the requester, never a fact. Printable, one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label(String);

/// Text that is no label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LabelError {
    /// Nothing.
    Empty,
    /// Longer than [`Label::LONGEST`] characters.
    Long {
        /// How many.
        found: usize,
    },
    /// A control character, at this offset.
    Control {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for LabelError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("a label is not empty"),
            Self::Long { found } => write!(out, "a label is at most {} characters, not {found}", Label::LONGEST),
            Self::Control { at } => write!(out, "character {at} of the label is a control character"),
        }
    }
}

impl std::error::Error for LabelError {}

impl Label {
    /// The most characters a label has.
    pub const LONGEST: usize = 64;

    /// `text` as a label.
    pub fn parse(text: &str) -> Result<Self, LabelError> {
        let found = text.chars().count();
        if found == 0 {
            return Err(LabelError::Empty);
        }
        if found > Self::LONGEST {
            return Err(LabelError::Long { found });
        }
        match text.chars().position(char::is_control) {
            Some(at) => Err(LabelError::Control { at }),
            None => Ok(Self(text.to_owned())),
        }
    }

    /// The label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for Label {
    type Err = LabelError;

    fn from_str(text: &str) -> Result<Self, LabelError> {
        Self::parse(text)
    }
}

impl fmt::Display for Label {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// Where a key stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum State {
    /// It signs.
    Live,
    /// It was revoked.
    Revoked {
        /// When.
        at: Timestamp,
        /// Why.
        why: Why,
    },
    /// It ran out.
    Expired {
        /// When it did.
        at: Timestamp,
    },
}

/// A key an account holds. The private half is not here and never was: the server stores the
/// public key and nothing that signs in.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Key {
    /// Its id, derived from the public key.
    pub id: KeyId,
    /// The account it acts for.
    pub account: Id<AccountKind>,
    /// What it is for.
    pub kind: Kind,
    /// Its type.
    pub key_type: KeyType,
    /// The public key.
    pub public: PublicKey,
    /// What the person called it.
    pub label: Label,
    /// What it may do, and where.
    pub grant: Grant,
    /// When it was approved.
    pub created: Timestamp,
    /// The account that approved it.
    pub approved_by: Id<AccountKind>,
    /// When it ends.
    pub expires: Timestamp,
    /// The day it last signed a request, to the day and no finer.
    pub last_used: Option<Date>,
    /// When and why it was revoked.
    pub revoked: Option<(Timestamp, Why)>,
}

impl Key {
    /// The columns a read of a key selects.
    pub(crate) const COLUMNS: [table::Key; 15] = [
        table::Key::Id,
        table::Key::AccountId,
        table::Key::Kind,
        table::Key::KeyType,
        table::Key::PublicKey,
        table::Key::Label,
        table::Key::Rights,
        table::Key::ScopeKind,
        table::Key::SiteId,
        table::Key::CreatedAt,
        table::Key::ApprovedBy,
        table::Key::ExpiresAt,
        table::Key::LastUsedOn,
        table::Key::RevokedAt,
        table::Key::RevokedWhy,
    ];

    /// Where the key stands at `now`: revoked before expired, since a revocation is a decision.
    pub fn state(&self, now: Timestamp) -> State {
        match self.revoked {
            Some((at, why)) => State::Revoked { at, why },
            None if self.expires <= now => State::Expired { at: self.expires },
            None => State::Live,
        }
    }

    /// The way of signing in this key is, for the account's list.
    pub fn credential(&self) -> Credential {
        match (self.kind, self.grant.scope) {
            (Kind::Deploy, Scope::Site(site)) => Credential::DeployKey { key: self.id, site },
            _ => Credential::DeviceKey { key: self.id },
        }
    }

    /// What this key may do, on its own and before a person's grants narrow it.
    pub fn grants(&self) -> Grants {
        Grants::new(vec![self.grant])
    }

    /// The scope as the two columns store it.
    pub(crate) fn scope_columns(scope: Scope) -> (&'static str, Option<String>) {
        match scope {
            Scope::Account(_) => ("account", None),
            Scope::Site(site) => ("site", Some(site.to_string())),
        }
    }
}

impl TryFrom<&Row> for Key {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let id = fields.key(table::Key::Id)?;
        let account = fields.id::<AccountKind>(table::Key::AccountId)?;
        let scope = match fields.text(table::Key::ScopeKind)?.as_str() {
            "account" => Scope::Account(account),
            _ => Scope::Site(
                fields.maybe_id::<SiteKind>(table::Key::SiteId)?.ok_or(Unreadable::Text { column: "site_id" })?,
            ),
        };
        let rights = Rights::read(&fields.text(table::Key::Rights)?)
            .map_err(|source| Unreadable::Name { column: "rights", found: source.written })?;
        let bytes: Vec<u8> = row.get(table::Key::PublicKey)?;
        let public = <[u8; 32]>::try_from(bytes.as_slice())
            .map(PublicKey::new)
            .map_err(|_| Unreadable::Digest { column: "public_key" })?;
        Ok(Self {
            id,
            account,
            kind: fields.name(table::Key::Kind, &Kind::NAMES)?,
            key_type: fields.name(table::Key::KeyType, &KeyType::NAMES)?,
            public,
            label: Label::parse(&fields.text(table::Key::Label)?).map_err(|_| Unreadable::Text { column: "label" })?,
            grant: Grant { scope, rights },
            created: fields.time(table::Key::CreatedAt)?,
            approved_by: fields.id(table::Key::ApprovedBy)?,
            expires: fields.time(table::Key::ExpiresAt)?,
            last_used: fields.maybe_day(table::Key::LastUsedOn)?,
            revoked: match fields.when(table::Key::RevokedAt)? {
                Some(at) => Some((at, fields.name(table::Key::RevokedWhy, &Why::NAMES)?)),
                None => None,
            },
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_label_is_one_printable_line() {
        assert!(Label::parse("laptop").is_ok());
        assert_eq!(Label::parse(""), Err(LabelError::Empty));
        assert_eq!(Label::parse("a\nb"), Err(LabelError::Control { at: 1 }));
        assert!(matches!(Label::parse(&"x".repeat(65)), Err(LabelError::Long { found: 65 })));
    }
}
