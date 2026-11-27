//! `ink status`: what the site serves, what it is building, and how it is doing.

use std::path::PathBuf;

use clap::Args;

use super::view::Standing;
use crate::deploy::Remote;
use crate::error::Error;
use crate::exit::Exit;
use crate::output::Io;

/// `ink status`.
#[derive(Debug, Args)]
pub struct Status {
    /// Which site, and how to reach it.
    #[command(flatten)]
    pub remote: Remote,
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
}

impl Status {
    /// Says how the folder's site is doing.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.remote.folder(&self.dir)?;
        let overview = calls.block(calls.api.overview())?;
        let standing = Standing::of(&overview);
        io.result(&standing)?;
        io.json(&standing)?;
        Ok(Exit::Done)
    }
}
