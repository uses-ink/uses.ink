//! The names TOML gives the values of closed enums, shared by reading and writing so the two
//! cannot drift.

use model::{Layout, PageKind};

/// `layout = "..."`.
pub(crate) const LAYOUTS: &[(&str, Layout)] =
    &[("blog", Layout::Blog), ("vault", Layout::Vault), ("docs", Layout::Docs), ("flat", Layout::Flat)];

/// `kind = "..."`.
pub(crate) const KINDS: &[(&str, PageKind)] =
    &[("home", PageKind::Home), ("post", PageKind::Post), ("page", PageKind::Page)];

/// The value named `text`.
pub(crate) fn named<T: Copy>(names: &[(&str, T)], text: &str) -> Option<T> {
    names.iter().find(|(name, _)| *name == text).map(|(_, value)| *value)
}

/// The name of `value`.
pub(crate) fn name_of<T: PartialEq>(names: &[(&'static str, T)], value: &T) -> Option<&'static str> {
    names.iter().find(|(_, v)| v == value).map(|(name, _)| *name)
}
