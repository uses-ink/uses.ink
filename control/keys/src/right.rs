//! What a credential may do, and where (`docs/auth.md` section 12).

use std::fmt;

use control_rows::id::{Account as AccountKind, Id, Site as SiteKind};

/// What a credential may do. A closed list: adding one is a code change and a decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Right {
    /// List and read sites, deploys, builds, logs and exports.
    Read,
    /// Take a built folder, request a build, roll back and resume.
    Deploy,
    /// Add a site and change its settings.
    Edit,
    /// Remove a site, delete the account.
    Destroy,
    /// List keys and sessions and revoke a device or deploy key.
    Keys,
    /// Invite, remove and change members (orgs).
    Members,
}

impl Right {
    /// Every right, in the order they are listed.
    pub const ALL: [Self; 6] = [Self::Read, Self::Deploy, Self::Edit, Self::Destroy, Self::Keys, Self::Members];

    /// The names a column and the wire store.
    pub const NAMES: [(&'static str, Self); 6] = [
        ("read", Self::Read),
        ("deploy", Self::Deploy),
        ("edit", Self::Edit),
        ("destroy", Self::Destroy),
        ("keys", Self::Keys),
        ("members", Self::Members),
    ];

    /// The name it is stored and written as.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Deploy => "deploy",
            Self::Edit => "edit",
            Self::Destroy => "destroy",
            Self::Keys => "keys",
            Self::Members => "members",
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Read => 1,
            Self::Deploy => 2,
            Self::Edit => 4,
            Self::Destroy => 8,
            Self::Keys => 16,
            Self::Members => 32,
        }
    }
}

impl fmt::Display for Right {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// A name that is no right.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownRight {
    /// What was written.
    pub written: String,
}

impl fmt::Display for UnknownRight {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = Right::NAMES.iter().map(|(name, _)| *name).collect();
        write!(out, "`{}` is no right: the rights are {}", self.written, names.join(", "))
    }
}

impl std::error::Error for UnknownRight {}

impl std::str::FromStr for Right {
    type Err = UnknownRight;

    fn from_str(text: &str) -> Result<Self, UnknownRight> {
        Self::NAMES
            .iter()
            .find(|(name, _)| *name == text)
            .map(|(_, right)| *right)
            .ok_or_else(|| UnknownRight { written: text.to_owned() })
    }
}

/// A set of [`Right`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Rights(u8);

impl Rights {
    /// No right.
    pub const NONE: Self = Self(0);

    /// Every right.
    pub const EVERY: Self = Self(63);

    /// What a device key has when nothing narrower is asked: `read`, `deploy`, `edit` and `keys`,
    /// not `destroy` and not `members`. A site is deleted in the dashboard, with a passkey.
    pub const DEVICE: Self = Self(Right::Read.bit() | Right::Deploy.bit() | Right::Edit.bit() | Right::Keys.bit());

    /// What a deploy key has when nothing else is asked: `deploy` on its one site.
    pub const DEPLOY: Self = Self(Right::Deploy.bit());

    /// The set of `rights`.
    pub fn of(rights: &[Right]) -> Self {
        Self(rights.iter().fold(0, |set, right| set | right.bit()))
    }

    /// Whether `right` is in the set.
    pub const fn has(self, right: Right) -> bool {
        self.0 & right.bit() != 0
    }

    /// Whether every right of `other` is in this set.
    pub const fn covers(self, other: Self) -> bool {
        other.0 & !self.0 == 0
    }

    /// The rights in both.
    #[must_use]
    pub const fn both(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// The rights in either.
    #[must_use]
    pub const fn either(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether the set is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The rights, in the order [`Right::ALL`] lists them.
    pub fn iter(self) -> impl Iterator<Item = Right> {
        Right::ALL.into_iter().filter(move |right| self.has(*right))
    }

    /// The set as a column stores it: names joined by `,`.
    pub fn stored(self) -> String {
        self.iter().map(Right::name).collect::<Vec<_>>().join(",")
    }

    /// The set a column stored, or the name it does not know.
    pub fn read(text: &str) -> Result<Self, UnknownRight> {
        text.split(',')
            .filter(|name| !name.is_empty())
            .try_fold(Self::NONE, |set, name| name.parse::<Right>().map(|right| set.either(Self::of(&[right]))))
    }
}

impl std::str::FromStr for Rights {
    type Err = UnknownRight;

    fn from_str(text: &str) -> Result<Self, UnknownRight> {
        Self::read(text)
    }
}

impl fmt::Display for Rights {
    /// `read, deploy`, or `none`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return out.write_str("none");
        }
        out.write_str(&self.iter().map(Right::name).collect::<Vec<_>>().join(", "))
    }
}

/// Where a right applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Scope {
    /// Every site of the account, and the account itself.
    Account(Id<AccountKind>),
    /// One site.
    Site(Id<SiteKind>),
}

/// What a call is about: the account, and the site when it is about one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// The account the call acts on.
    pub account: Id<AccountKind>,
    /// The site, for a call about one site.
    pub site: Option<Id<SiteKind>>,
}

/// Some rights, in a scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grant {
    /// Where they apply.
    pub scope: Scope,
    /// What they are.
    pub rights: Rights,
}

impl Grant {
    /// Whether the scope covers `target`: `Account(a)` covers every site of `a`, `Site(s)` covers `s`.
    pub fn covers(&self, target: &Target) -> bool {
        match (self.scope, target.site) {
            (Scope::Account(account), _) => account == target.account,
            (Scope::Site(site), Some(wanted)) => site == wanted,
            (Scope::Site(_), None) => false,
        }
    }
}

/// A set of grants. Rights compose by **union** within it and **nothing denies**: an effective
/// right is always explained by naming the grant that gives it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Grants(Vec<Grant>);

impl Grants {
    /// The grants of `list`.
    pub const fn new(list: Vec<Grant>) -> Self {
        Self(list)
    }

    /// What the owner of a self-host has: every right on the account (roles and orgs arrive with
    /// the hosted product; one account has the `Owner` role only).
    pub fn owner(account: Id<AccountKind>) -> Self {
        Self(vec![Grant { scope: Scope::Account(account), rights: Rights::EVERY }])
    }

    /// The rights these grants give on `target`.
    pub fn on(&self, target: &Target) -> Rights {
        self.0.iter().filter(|grant| grant.covers(target)).fold(Rights::NONE, |all, grant| all.either(grant.rights))
    }

    /// The grants.
    pub fn list(&self) -> &[Grant] {
        &self.0
    }
}

/// What a principal may do: its own grants and, for a device key, the grants of the person it
/// acts for. A device key is an attenuation: it may do what **both** allow, so demoting a person
/// narrows their keys the same moment. A deploy key belongs to the account (a service identity):
/// its own grants stand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effective {
    own: Grants,
    bound: Option<Grants>,
}

impl Effective {
    /// A principal with `own` grants, bound by none.
    pub const fn own(own: Grants) -> Self {
        Self { own, bound: None }
    }

    /// A principal with `own` grants that may do no more than `person` may.
    pub const fn within(own: Grants, person: Grants) -> Self {
        Self { own, bound: Some(person) }
    }

    /// The rights on `target`.
    pub fn on(&self, target: &Target) -> Rights {
        let own = self.own.on(target);
        self.bound.as_ref().map_or(own, |person| own.both(person.on(target)))
    }

    /// Whether `right` is allowed on `target`.
    pub fn allows(&self, right: Right, target: &Target) -> bool {
        self.on(target).has(right)
    }

    /// The principal's own grants.
    pub const fn grants(&self) -> &Grants {
        &self.own
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn account(n: u8) -> Id<AccountKind> {
        Id::from_bytes([n; 16])
    }

    fn site(n: u8) -> Id<SiteKind> {
        Id::from_bytes([n; 16])
    }

    #[test]
    fn rights_are_a_set_that_round_trips_through_its_column() {
        let set = Rights::of(&[Right::Deploy, Right::Read]);
        assert_eq!(set.stored(), "read,deploy");
        assert_eq!(Rights::read("read,deploy"), Ok(set));
        assert!(Rights::read("read,fly").is_err());
        assert_eq!(set.to_string(), "read, deploy");
        assert_eq!(Rights::NONE.to_string(), "none");
        assert!(Rights::EVERY.covers(set) && !set.covers(Rights::EVERY));
        assert_eq!(Rights::EVERY.iter().count(), Right::ALL.len());
    }

    #[test]
    fn the_defaults_are_the_least_that_works() {
        assert_eq!(Rights::DEVICE.stored(), "read,deploy,edit,keys");
        assert!(!Rights::DEVICE.has(Right::Destroy) && !Rights::DEVICE.has(Right::Members));
        assert_eq!(Rights::DEPLOY.stored(), "deploy");
    }

    #[test]
    fn grants_union_and_nothing_denies() {
        let (a, s1, s2) = (account(1), site(1), site(2));
        let grants = Grants::new(vec![
            Grant { scope: Scope::Account(a), rights: Rights::of(&[Right::Read]) },
            Grant { scope: Scope::Site(s1), rights: Rights::of(&[Right::Deploy]) },
            Grant { scope: Scope::Site(s1), rights: Rights::of(&[Right::Edit]) },
        ]);
        let on = |site| grants.on(&Target { account: a, site });
        assert_eq!(on(Some(s1)), Rights::of(&[Right::Read, Right::Deploy, Right::Edit]));
        assert_eq!(on(Some(s2)), Rights::of(&[Right::Read]));
        assert_eq!(on(None), Rights::of(&[Right::Read]));
        assert_eq!(
            grants.on(&Target { account: account(2), site: Some(s1) }),
            Rights::of(&[Right::Deploy, Right::Edit])
        );
    }

    #[test]
    fn a_device_key_is_the_person_s_grants_narrowed_and_a_deploy_key_stands_alone() {
        let (a, s) = (account(1), site(1));
        let target = Target { account: a, site: Some(s) };
        let key = Grants::new(vec![Grant { scope: Scope::Account(a), rights: Rights::DEVICE }]);
        let demoted = Grants::new(vec![Grant { scope: Scope::Account(a), rights: Rights::of(&[Right::Read]) }]);
        assert!(Effective::within(key.clone(), Grants::owner(a)).allows(Right::Deploy, &target));
        assert!(!Effective::within(key.clone(), demoted.clone()).allows(Right::Deploy, &target));
        assert!(Effective::within(key.clone(), demoted).allows(Right::Read, &target));
        assert!(!Effective::within(key, Grants::owner(a)).allows(Right::Destroy, &target));
        let deploy = Effective::own(Grants::new(vec![Grant { scope: Scope::Site(s), rights: Rights::DEPLOY }]));
        assert!(deploy.allows(Right::Deploy, &target));
        assert!(!deploy.allows(Right::Read, &target));
        assert!(!deploy.allows(Right::Deploy, &Target { account: a, site: Some(site(2)) }));
        assert!(!deploy.allows(Right::Deploy, &Target { account: a, site: None }));
    }
}
