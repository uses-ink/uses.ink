//! The host side of a run: the shell that starts commands, the npm packages, and the Worker crate's build.

mod build;
mod npm;
mod shell;

pub(crate) use build::Crate;
pub(crate) use npm::Packages;
pub(crate) use shell::Shell;
