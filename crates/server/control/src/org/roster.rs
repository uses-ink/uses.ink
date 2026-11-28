//! Who is in an organization, for a caller that has a database and no [`Control`]: the sign-in
//! door, which approves a key into an organization on behalf of a member.

use std::fmt;

use db::Db;
use sea_query::{Expr, ExprTrait, Query};

use super::{Member, Role};
use crate::id::{Account as AccountKind, Id};
use crate::keys::{Right, Rights};
use crate::{Unreadable, table};

/// The members of organizations, read from a database.
#[derive(Clone, Debug)]
pub struct Roster<D>(D);

/// Why a person may not give a key to an organization.
#[derive(Debug)]
#[non_exhaustive]
pub enum Seated<E> {
    /// They are no member of it.
    NotMember {
        /// The organization.
        org: Id<AccountKind>,
        /// The person.
        account: Id<AccountKind>,
    },
    /// Their role does not hold the right.
    Role {
        /// The organization.
        org: Id<AccountKind>,
        /// What they are there.
        role: Role,
        /// What was needed.
        needs: Right,
    },
    /// The member row could not be read.
    Unreadable(Unreadable),
    /// The database failed.
    Store(db::Error<E>),
}

impl<E> fmt::Display for Seated<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotMember { org, account } => write!(out, "{account} is not a member of {org}"),
            Self::Role { org, role, needs } => write!(out, "a {role} of {org} has no `{}` right", needs.name()),
            Self::Unreadable(_) => out.write_str("the member row could not be read"),
            Self::Store(_) => out.write_str("the members could not be read"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Seated<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable(source) => Some(source),
            Self::Store(source) => Some(source),
            _ => None,
        }
    }
}

impl<D: Db> Roster<D> {
    /// The members of the organizations in `db`.
    pub const fn new(db: D) -> Self {
        Self(db)
    }

    /// What a key `person` gives to `org` may hold at most: the role's rights, when the role may
    /// deploy. A key of an organization belongs to it and outlives the member (decision 0095), so
    /// it has no more than the person who approved it had.
    ///
    /// # Errors
    ///
    /// [`Seated`] when the person is no member, their role cannot deploy, or a read failed.
    pub async fn allowed(&self, org: Id<AccountKind>, person: Id<AccountKind>) -> Result<Rights, Seated<D::Error>> {
        let select = Query::select()
            .columns(Member::COLUMNS)
            .from(table::Member::Table)
            .and_where(Expr::col(table::Member::OrgId).eq(org.to_string()))
            .and_where(Expr::col(table::Member::AccountId).eq(person.to_string()))
            .to_owned();
        let row = self.0.first(select.into()).await.map_err(Seated::Store)?;
        let member = row.map(|row| Member::read(&row)).transpose().map_err(Seated::Unreadable)?;
        let Some(member) = member else { return Err(Seated::NotMember { org, account: person }) };
        let rights = member.role.rights();
        if rights.has(Right::Deploy) {
            Ok(rights)
        } else {
            Err(Seated::Role { org, role: member.role, needs: Right::Deploy })
        }
    }
}
