//! One module per command, or per family of commands: what each does once its arguments are
//! parsed, and the pieces of the build they share.

pub(crate) mod auth;
pub(crate) mod build;
pub(crate) mod bundle;
pub(crate) mod change;
pub(crate) mod component;
pub(crate) mod explain;
pub(crate) mod format;
pub(crate) mod images;
pub(crate) mod pin;
pub(crate) mod preview;
pub(crate) mod publish;
pub(crate) mod remote;
pub(crate) mod select;
pub(crate) mod serve;
pub(crate) mod source;
