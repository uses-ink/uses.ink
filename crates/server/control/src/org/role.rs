//! The roles of an organization (`docs/auth.md` section 12): named sets of rights, in code and
//! not in rows, so a role is the same everywhere and no one can write a custom one.

use std::fmt;
use std::str::FromStr;

use crate::keys::{Right, Rights};

/// What a member may do in an organization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Role {
    /// Everything, including members and destroying.
    Owner,
    /// Everything but destroying.
    Admin,
    /// Read, deploy and edit.
    Writer,
    /// Read.
    Reader,
}

impl Role {
    /// Every role, most able first.
    pub const ALL: [Self; 4] = [Self::Owner, Self::Admin, Self::Writer, Self::Reader];

    /// The names a column and the wire store.
    pub const NAMES: [(&'static str, Self); 4] =
        [("owner", Self::Owner), ("admin", Self::Admin), ("writer", Self::Writer), ("reader", Self::Reader)];

    /// The name it is stored and written as.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Writer => "writer",
            Self::Reader => "reader",
        }
    }

    /// The rights the role gives on the organization's sites.
    pub fn rights(self) -> Rights {
        match self {
            Self::Owner => Rights::EVERY,
            Self::Admin => Rights::of(&[Right::Read, Right::Deploy, Right::Edit, Right::Keys, Right::Members]),
            Self::Writer => Rights::of(&[Right::Read, Right::Deploy, Right::Edit]),
            Self::Reader => Rights::of(&[Right::Read]),
        }
    }

    /// Whether a member with this role may give `other` to someone, or take it away: members
    /// are managed by a role that holds `Members`, and only an owner makes or unmakes an owner,
    /// so an admin cannot raise themselves or remove the people above them.
    pub fn manages(self, other: Self) -> bool {
        self.rights().has(Right::Members) && (self == Self::Owner || other != Self::Owner)
    }

    /// The roles that may manage `other`.
    pub fn managers(other: Self) -> Vec<Self> {
        Self::ALL.into_iter().filter(|role| role.manages(other)).collect()
    }
}

impl fmt::Display for Role {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// A name that is no role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownRole {
    /// What was written.
    pub written: String,
}

impl fmt::Display for UnknownRole {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = Role::NAMES.iter().map(|(name, _)| *name).collect();
        write!(out, "`{}` is no role: the roles are {}", self.written, names.join(", "))
    }
}

impl std::error::Error for UnknownRole {}

impl FromStr for Role {
    type Err = UnknownRole;

    fn from_str(text: &str) -> Result<Self, UnknownRole> {
        Self::NAMES
            .iter()
            .find(|(name, _)| *name == text)
            .map(|(_, role)| *role)
            .ok_or_else(|| UnknownRole { written: text.chars().take(40).collect() })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_role_table_of_the_auth_doc_holds() {
        assert_eq!(Role::Owner.rights(), Rights::EVERY);
        assert!(!Role::Admin.rights().has(Right::Destroy) && Role::Admin.rights().has(Right::Members));
        assert_eq!(Role::Writer.rights(), Rights::of(&[Right::Read, Right::Deploy, Right::Edit]));
        assert_eq!(Role::Reader.rights(), Rights::of(&[Right::Read]));
        // a ladder: each role holds the rights of the one below
        for pair in Role::ALL.windows(2) {
            assert!(pair[0].rights().covers(pair[1].rights()), "{} must cover {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn only_a_role_with_members_manages_and_only_an_owner_manages_an_owner() {
        for role in Role::ALL {
            for target in Role::ALL {
                let expect = role.rights().has(Right::Members) && (role == Role::Owner || target != Role::Owner);
                assert_eq!(role.manages(target), expect, "{role} over {target}");
            }
        }
        assert!(!Role::Writer.manages(Role::Reader) && !Role::Admin.manages(Role::Owner));
        assert_eq!(Role::managers(Role::Owner), [Role::Owner]);
        assert_eq!(Role::managers(Role::Writer), [Role::Owner, Role::Admin]);
    }

    #[test]
    fn names_round_trip() {
        for role in Role::ALL {
            assert_eq!(role.name().parse::<Role>().unwrap(), role);
        }
        assert!("root".parse::<Role>().is_err());
    }
}
