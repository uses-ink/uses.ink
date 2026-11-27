//! `ink export`: download what the site serves, or what was pushed, into a folder.
//!
//! The archive is read in memory within limits the flags set, written into a folder beside the
//! target, and renamed into place, so a failure leaves no partial folder.

use std::path::{Path, PathBuf};

use clap::Args;
use edge_api::tar::{Limits, Tree};

use super::view::Exported;
use model::DeployId;

use crate::deploy::{Calls, Failure, Remote};
use crate::error::Error;
use crate::exit::Exit;
use crate::output::Io;
use crate::stage::{StageLine, Verb};
use crate::term::Answer;

/// `ink export [DIR]`.
#[derive(Debug, Args)]
pub struct Export {
    /// The folder to write: new, or empty.
    #[arg(value_name = "DIR", default_value = "export")]
    pub target: PathBuf,
    /// Download the files the last push sent, not the site they built.
    #[arg(long, conflicts_with = "deploy")]
    pub source: bool,
    /// Download this deploy, by the start of its id, not the live one.
    #[arg(long, value_name = "DEPLOY")]
    pub deploy: Option<String>,
    /// Most files the archive may hold.
    #[arg(long, value_name = "N", default_value_t = 100_000)]
    pub max_files: u64,
    /// Most bytes one file may hold.
    #[arg(long, value_name = "BYTES", default_value_t = 100 * 1024 * 1024)]
    pub max_file: u64,
    /// Most bytes all the files may hold.
    #[arg(long, value_name = "BYTES", default_value_t = 2 * 1024 * 1024 * 1024)]
    pub max_bytes: u64,
    /// Longest file name, in bytes.
    #[arg(long, value_name = "BYTES", default_value_t = 1024)]
    pub max_name: usize,
    /// Which site, and how to reach it.
    #[command(flatten)]
    pub remote: Remote,
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
}

/// A folder being filled beside its final place.
#[derive(Debug)]
struct Staging {
    path: PathBuf,
}

impl Staging {
    /// A new folder beside `target`.
    fn beside(target: &Path) -> Result<Self, Failure> {
        let name = target.file_name().map_or_else(|| "export".into(), |name| name.to_string_lossy().into_owned());
        let parent = target.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|source| Failure::Placing { path: parent.to_path_buf(), source })?;
        let path = parent.join(format!(".{name}.partial-{}", std::process::id()));
        if path.exists() {
            std::fs::remove_dir_all(&path).map_err(|source| Failure::Placing { path: path.clone(), source })?;
        }
        std::fs::create_dir(&path).map_err(|source| Failure::Placing { path: path.clone(), source })?;
        Ok(Self { path })
    }

    /// Writes every file of `tree`.
    fn fill(&self, tree: &Tree) -> Result<u64, Failure> {
        let mut bytes = 0_u64;
        for (path, content) in tree.files() {
            let at = self.path.join(path.as_str());
            let placing = |source| Failure::Placing { path: at.clone(), source };
            if let Some(parent) = at.parent() {
                std::fs::create_dir_all(parent).map_err(placing)?;
            }
            std::fs::write(&at, content).map_err(placing)?;
            bytes = bytes.saturating_add(content.len() as u64);
        }
        Ok(bytes)
    }

    /// Moves the folder to `target`, which is free or an empty folder.
    fn settle(&self, target: &Path) -> Result<(), Failure> {
        let placing = |source| Failure::Placing { path: target.to_path_buf(), source };
        if target.is_dir() {
            std::fs::remove_dir(target).map_err(placing)?;
        }
        std::fs::rename(&self.path, target).map_err(placing)
    }

    /// Takes back what was written, which is this folder and nothing else.
    fn discard(self) {
        // Nothing to say if this fails too: the failure that led here is the one reported.
        std::fs::remove_dir_all(&self.path).ok();
    }
}

impl Export {
    /// Whether `target` may be written: it is not there, or it is an empty folder.
    fn free(target: &Path) -> Result<(), Failure> {
        match std::fs::read_dir(target) {
            Ok(entries) => match entries.count() {
                0 => Ok(()),
                _ => Err(Failure::Occupied { path: target.to_path_buf() }),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(Failure::Placing { path: target.to_path_buf(), source }),
        }
    }

    /// The deploy `--deploy` names, by the start of its id.
    fn pick(&self, calls: &Calls) -> Result<Option<DeployId>, Failure> {
        let Some(written) = &self.deploy else { return Ok(None) };
        let all = calls.block(calls.api.deploys())?;
        let mut found = all.iter().filter(|row| row.id.to_string().starts_with(written.as_str()));
        match (found.next(), found.count()) {
            (Some(row), 0) => Ok(Some(row.id)),
            (Some(_), more) => Err(Failure::Ambiguous { written: written.clone(), count: more + 1 }),
            (None, _) => Err(Failure::Unknown { written: written.clone() }),
        }
    }

    /// Downloads the archive and places its files.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.remote.folder(&self.dir)?;
        let target = if self.target.is_absolute() { self.target.clone() } else { self.dir.join(&self.target) };
        Self::free(&target)?;
        let deploy = self.pick(&calls)?;
        let archive =
            if self.source { calls.block(calls.api.bundle())? } else { calls.block(calls.api.export(deploy))? };
        let limits = Limits { files: self.max_files, file: self.max_file, total: self.max_bytes, name: self.max_name };
        let tree = Tree::read(&archive, &limits).map_err(|source| Failure::Archive { source })?;
        let staging = Staging::beside(&target)?;
        let placed = staging.fill(&tree).and_then(|bytes| {
            Self::free(&target)?;
            Ok(bytes)
        });
        let bytes = match placed {
            Ok(bytes) => bytes,
            Err(failure) => {
                staging.discard();
                return Err(failure.into());
            }
        };
        if let Err(failure) = staging.settle(&target) {
            staging.discard();
            return Err(failure.into());
        }
        let exported = Exported {
            schema: Exported::SCHEMA,
            what: if self.source { "source" } else { "site" },
            dir: target,
            files: tree.len() as u64,
            bytes,
        };
        io.stage(&StageLine::new(Verb::Exported, exported.said()))?;
        io.result(&Answer::of(exported.dir.display()))?;
        io.json(&exported)?;
        Ok(Exit::Done)
    }
}
