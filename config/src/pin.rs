//! A pin: a value an author fixed in `ink.toml`, and where it is written. Where a code is pinned
//! comes from its name: `site.home` is key `home` of table `site`; a per-file code adds the
//! file's path between the two.

use model::decision::Scope;
use model::{AnyCode, Code, Family, Span};

/// A value pinned for code `C` and where it is written (`key = value`): a decision's
/// `Evidence::Config { span }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pinned<C: Code> {
    /// The value the author pinned.
    pub value: C::Value,
    /// The whole `key = value`.
    pub span: Span,
}

/// The family of [`Pinned`], so the pins of every code sit in one map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pins;

impl Family for Pins {
    type Of<C: Code> = Pinned<C>;
}

/// Where a code is pinned: `page.title` is key `title` of table `page`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    /// The code pinned there.
    pub(crate) code: AnyCode,
    /// `page`.
    pub(crate) table: &'static str,
    /// `title`.
    pub(crate) key: &'static str,
}

/// Every code's name is `table.key` (conventions.md, Grammar); a build with a code whose name is
/// not fails here, so [`Place::from`] never meets one.
const _: () = {
    let mut codes = AnyCode::ALL;
    while let [code, rest @ ..] = codes {
        let mut name = code.name().as_bytes();
        let mut dotted = false;
        while let [byte, rest @ ..] = name {
            dotted |= *byte == b'.';
            name = rest;
        }
        assert!(dotted, "a decision code's name is `table.key`");
        codes = rest;
    }
};

impl From<AnyCode> for Place {
    fn from(code: AnyCode) -> Self {
        // the check above makes every name `table.key`
        let (table, key) = code.name().split_once('.').unwrap_or((code.name(), ""));
        Self { code, table, key }
    }
}

impl Place {
    /// Every place, in decision order.
    pub(crate) fn all() -> impl Iterator<Item = Self> {
        AnyCode::ALL.iter().copied().map(Self::from)
    }

    /// The place at `key` of `table` for subjects of `scope`, if a code is pinned there.
    pub(crate) fn find(table: &str, key: &str, scope: Scope) -> Option<Self> {
        Self::all().find(|place| place.table == table && place.key == key && place.code.scope() == scope)
    }

    /// Whether `table` holds pins on subjects of `scope`.
    pub(crate) fn holds(table: &str, scope: Scope) -> bool {
        Self::all().any(|place| place.table == table && place.code.scope() == scope)
    }
}

/// A dotted key as TOML writes it: `page."posts/x.md".title`. Its `Display` is in `write`,
/// which owns `toml_edit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyPath<'a>(pub(crate) Vec<&'a str>);

impl<'a> KeyPath<'a> {
    /// The same path, one key deeper.
    pub(crate) fn and(&self, key: &'a str) -> Self {
        let mut keys = self.0.clone();
        keys.push(key);
        Self(keys)
    }
}
