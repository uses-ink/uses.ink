//! The command surface: the arguments clap parses, the tree of commands every help page, manual
//! and completion is drawn from, and the dispatch of a parsed command to its module.

pub(crate) mod args;
pub(crate) mod completions;
pub(crate) mod dispatch;
pub(crate) mod help;
pub(crate) mod man;
pub(crate) mod tree;
