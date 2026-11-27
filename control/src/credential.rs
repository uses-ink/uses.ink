//! Credentials: secrets a site's source needs, sealed at rest. This is the metadata; the sealed
//! bytes are never read into a value that can be printed.

use db::Row;
use jiff::Timestamp;
use secret::Plain;

use crate::fields::Fields;
use crate::id::{Credential as Kind, Id, Site};
use crate::Unreadable;

/// What a credential is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Use {
    /// A token that reads a private git repository.
    GitToken,
    /// The secret a forge signs its webhooks with.
    WebhookSecret,
}

impl Use {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 2] = [("git-token", Self::GitToken), ("webhook-secret", Self::WebhookSecret)];

    /// The text this use is stored as.
    pub(crate) fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("git-token", |(name, _)| name)
    }
}

/// A credential to store.
#[derive(Debug)]
pub struct New {
    /// Its id, drawn by the caller. The sealed value is bound to it.
    pub id: Id<Kind>,
    /// The site it belongs to.
    pub site: Id<Site>,
    /// What it is for.
    pub purpose: Use,
    /// The secret itself, which is zeroed when this is dropped.
    pub secret: Plain,
    /// What time it is.
    pub at: Timestamp,
}

/// What is known about a stored credential, without the credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Credential {
    /// Its id.
    pub id: Id<Kind>,
    /// The site it belongs to.
    pub site: Id<Site>,
    /// What it is for.
    pub purpose: Use,
    /// When it was stored.
    pub created: Timestamp,
}

impl TryFrom<&Row> for Credential {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            id: fields.id("id")?,
            site: fields.id("site_id")?,
            purpose: fields.name("kind", &Use::NAMES)?,
            created: fields.time("created_at")?,
        })
    }
}
