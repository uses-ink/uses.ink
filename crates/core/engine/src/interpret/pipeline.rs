//! The stages of interpreting, in the order they run: the tree, the vault's settings, the files,
//! the pages, the links, the site and the private-source guard, each a [`Stage`] recording into
//! the journal.
//!
//! [`Stage`]: stage::Stage

pub(super) mod file;
pub(super) mod guard;
pub(super) mod link;
pub(super) mod page;
pub(super) mod site;
pub(super) mod stage;
pub(super) mod tree;
pub(super) mod vault;
