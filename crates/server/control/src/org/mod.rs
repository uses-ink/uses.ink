//! Organizations and their members (decision 0095, `docs/auth.md` section 12): an organization
//! is an account that people are members of, each with a [`Role`].

mod migration;
mod role;

use db::{Row, Statement};
use jiff::Timestamp;
use sea_query::{Expr, Query};

use crate::fields::Fields;
use crate::id::{Account as AccountKind, Id};
use crate::{Unreadable, table};

pub use migration::Orged;
pub use role::{Role, UnknownRole};

/// An organization: an account with a name.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Org {
    /// Its account, which owns its sites and its deploy keys.
    pub id: Id<AccountKind>,
    /// What people call it.
    pub name: String,
    /// When it was made.
    pub created: Timestamp,
}

impl Org {
    /// The columns, in the order [`Org::read`] reads them.
    pub(crate) const COLUMNS: [table::Org; 3] = [table::Org::Id, table::Org::Name, table::Org::CreatedAt];

    /// An organization from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            id: fields.id(table::Org::Id)?,
            name: fields.text(table::Org::Name)?,
            created: fields.time(table::Org::CreatedAt)?,
        })
    }
}

/// An organization to make: its account id, its first owner and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewOrg {
    /// The organization's account, drawn by the caller.
    pub id: Id<AccountKind>,
    /// The person who makes it, and is its first owner.
    pub owner: Id<AccountKind>,
    /// Its name.
    pub name: String,
    /// When it is made.
    pub at: Timestamp,
}

/// A person's place in an organization.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Member {
    /// The organization.
    pub org: Id<AccountKind>,
    /// The person's account.
    pub account: Id<AccountKind>,
    /// What they may do there.
    pub role: Role,
    /// When they joined.
    pub since: Timestamp,
}

impl Member {
    /// The columns, in the order [`Member::read`] reads them.
    pub(crate) const COLUMNS: [table::Member; 4] =
        [table::Member::OrgId, table::Member::AccountId, table::Member::Role, table::Member::CreatedAt];

    /// A member from its row.
    pub(crate) fn read(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            org: fields.id(table::Member::OrgId)?,
            account: fields.id(table::Member::AccountId)?,
            role: fields.name(table::Member::Role, &Role::NAMES)?,
            since: fields.time(table::Member::CreatedAt)?,
        })
    }

    /// The statement that records this membership, only when `guard` selects a row.
    pub(crate) fn insert(
        &self,
        guard: Option<sea_query::SelectStatement>,
    ) -> Result<Statement, sea_query::error::Error> {
        let mut select = Query::select();
        select.exprs([
            Expr::val(self.org.to_string()),
            Expr::val(self.account.to_string()),
            Expr::val(self.role.name()),
            Expr::val(self.since.as_millisecond()),
        ]);
        if let Some(guard) = guard {
            select.and_where(Expr::exists(guard));
        }
        Ok(Query::insert()
            .into_table(table::Member::Table)
            .columns(Self::COLUMNS)
            .select_from(select)?
            .to_owned()
            .into())
    }
}

/// A key of an organization, with whether the member who approved it is still one.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct OrgKey {
    /// The key.
    pub key: crate::keys::Key,
    /// The member who approved it has left the organization. The key stays until an owner
    /// revokes it.
    pub creator_removed: bool,
}
