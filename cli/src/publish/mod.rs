//! The commands that publish a folder: `ink init`, `ink push`, `ink status`, `ink watch` and
//! `ink export` (decision 0077).
//!
//! They talk to the signed API of `edge/api` through the machinery in [`crate::deploy`], and know
//! their folder by the link file `ink init` wrote.

mod export;
mod follow;
mod init;
mod link;
mod push;
mod status;
pub(crate) mod view;
mod watch;

pub(crate) use export::Export;
pub(crate) use init::Init;
pub(crate) use link::Link;
pub(crate) use link::LinkError;
pub(crate) use push::Push;
pub(crate) use status::Status;
pub(crate) use watch::Watch;
