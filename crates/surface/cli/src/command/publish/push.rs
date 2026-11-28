//! `ink push`: send the folder to the site and wait for what it builds.
//!
//! The folder becomes a tree of paths and hashes. The server answers which blobs it lacks, they
//! go up in archives of bounded size, and the tree is sent again until the server has everything
//! and queues a build. Nothing is kept between runs but the files the server already holds, so
//! an interrupted push is resumed by running it again and sends only what is left.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use build::BuildId;
use build::{Leaf, Tree};
use clap::Args;
use edge_api::Probed;
use edge_api::tar::Writer;
use engine::Blobs;
use host_native::{Folder, Memory};
use model::{ContentPath, Hash};

use super::follow::Follow;
use super::view::Pushed;
use crate::command::source::DEFAULT_OUT;
use crate::deploy::{Calls, Failure, Remote, Short, Size};
use crate::output::Io;
use crate::output::error::{Error, SourceError};
use crate::output::exit::Exit;
use crate::output::stage::{Plural, StageLine, Verb};
use crate::output::term::{Answer, Phase};

/// What a tar header and its padding cost on top of a file's bytes.
const OVERHEAD: u64 = 1536;

/// `ink push`.
#[derive(Debug, Args)]
pub struct Push {
    /// Return when the build is queued, without waiting for it.
    #[arg(long)]
    pub no_wait: bool,
    /// Seconds to wait for the build before stopping; it keeps going on the server.
    #[arg(long, value_name = "SECONDS", default_value_t = 300)]
    pub wait: u64,
    /// Milliseconds between two looks at the build.
    #[arg(long, value_name = "MS", default_value_t = 1000)]
    pub poll: u64,
    /// Most bytes of files in one request; the server's own limit applies when it is smaller.
    #[arg(long, value_name = "BYTES")]
    pub batch: Option<u64>,
    /// Most files in one request; the server's own limit applies when it is smaller.
    #[arg(long, value_name = "N", default_value_t = 1000)]
    pub batch_files: u64,
    /// How many times the tree is sent and the files it lacks answered, before giving up.
    #[arg(long, value_name = "N", default_value_t = 3)]
    pub rounds: u32,
    /// Which site, and how to reach it.
    #[command(flatten)]
    pub remote: Remote,
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
}

/// The folder read: its tree, and the bytes behind each hash.
#[derive(Debug)]
struct Scanned {
    tree: Tree,
    store: Memory,
    /// The files of the tree by hash, so a blob that is in the folder twice is sent once.
    sizes: BTreeMap<Hash, u64>,
}

impl Scanned {
    /// Reads the folder `dir`, without the folders that are not content.
    fn read(dir: &std::path::Path) -> Result<Self, Error> {
        let mut store = Memory::default();
        let output: Vec<ContentPath> =
            DEFAULT_OUT.parse().ok().filter(|_| dir.join(DEFAULT_OUT).is_dir()).into_iter().collect();
        let read = Folder { output, ..Folder::new(dir) }.read(&mut store).map_err(SourceError::Refused)?;
        let snapshot = read.snapshot;
        let modified = |path: &ContentPath| snapshot.facts_of(path).and_then(|facts| facts.updated.exact()).copied();
        let leaves = snapshot
            .tree()
            .iter()
            .map(|entry| Leaf {
                path: entry.path.clone(),
                hash: entry.blob,
                size: entry.size,
                modified: modified(&entry.path),
            })
            .collect();
        if let Some(held) = Self::data(&leaves) {
            return Err(Failure::Holds { path: held.to_string() }.into());
        }
        let tree = Tree::of(leaves).map_err(|source| Failure::Tree { source })?;
        let sizes = tree.files().iter().map(|leaf| (leaf.hash, leaf.size.count())).collect();
        Ok(Self { tree, store, sizes })
    }

    /// The first file of an `ink serve` data folder among `leaves`: its database, its signing key
    /// or its operator keys, at any depth.
    fn data(leaves: &[Leaf]) -> Option<&ContentPath> {
        const NAMES: [&str; 3] = ["ink.db", "secret.key", "operator-keys"];
        leaves.iter().map(|leaf| &leaf.path).find(|path| path.segments().any(|segment| NAMES.contains(&segment)))
    }

    /// The archives that carry `wanted`, each within `limit` bytes of files unless one file is
    /// larger, which then goes alone and is the server's to refuse.
    fn batches(&self, wanted: &[Hash], (limit, most): (u64, u64)) -> Result<Vec<(u64, Vec<u8>)>, Failure> {
        let mut batches = Vec::new();
        let mut open: Option<(u64, u64, Writer)> = None;
        for hash in wanted {
            let size = self.sizes.get(hash).copied().unwrap_or_default().saturating_add(OVERHEAD);
            if open.as_ref().is_some_and(|(files, held, _)| *files >= most || held.saturating_add(size) > limit)
                && let Some((files, _, writer)) = open.take()
            {
                batches.push((files, writer.finish()));
            }
            let (files, held, writer) = open.get_or_insert_with(|| (0, 0, Writer::new(u64::MAX)));
            let bytes = self.store.read(hash).map_err(|_| Failure::Absent { build: format!("blob {hash}") })?;
            writer.file(&hash.to_string(), &bytes).map_err(|source| Failure::Archive { source })?;
            *files += 1;
            *held = held.saturating_add(size);
        }
        if let Some((files, _, writer)) = open {
            batches.push((files, writer.finish()));
        }
        Ok(batches)
    }
}

/// One round of sending: what is sent where, and how.
#[derive(Clone, Copy, Debug)]
struct Round<'a> {
    calls: &'a Calls,
    scanned: &'a Scanned,
    /// Most bytes of files in a request.
    limit: u64,
    /// Most files in a request.
    most: u64,
    /// How many files earlier rounds of this push sent.
    before: u64,
}

impl Push {
    /// A count as the plural of a noun takes it.
    fn count(files: u64) -> usize {
        usize::try_from(files).unwrap_or(usize::MAX)
    }

    /// `error` as the failure of a command whose build is queued: the site keeps it going.
    fn queued(build: BuildId, error: Error) -> Error {
        match error {
            Error::Deploy { source } => {
                Error::Deploy { source: Failure::Following { build: build.to_string(), source: Box::new(source) } }
            }
            other => other,
        }
    }

    /// Sends the folder and, unless told not to, waits for what the site builds of it.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.remote.folder(&self.dir)?;
        let scanned = Scanned::read(&self.dir)?;
        let (files, bytes) = (scanned.tree.files().len(), scanned.tree.total().count());
        io.stage(&StageLine::new(Verb::Read, format!("{}, {}", Plural(files, "file"), Size(bytes))))?;
        let (build, sent) = self.send(&calls, &scanned, io)?;
        let held = (files as u64).saturating_sub(sent);
        io.stage(&StageLine::new(Verb::Queued, format!("build {build}, {sent} sent, {held} already held")))?;
        let site = calls.api.site.to_string();
        let tree = Some(scanned.tree.hash().to_string());
        let counts = |mut pushed: Pushed| {
            pushed.tree.clone_from(&tree);
            pushed.files = Some(files as u64);
            pushed.sent = Some(sent);
            pushed.held = Some(held);
            pushed
        };
        if self.no_wait {
            let queued = Follow::queued(build);
            let pushed = counts(Follow::document(Pushed::PUSH, site, &queued, None));
            io.result(&Answer::of(build))?;
            io.json(&pushed)?;
            return Ok(Exit::Done);
        }
        let follow =
            Follow { calls: &calls, poll: Duration::from_millis(self.poll), wait: Duration::from_secs(self.wait) };
        let ended = follow.until(build, io).map_err(|error| Self::queued(build, error))?;
        let row = follow.settle(&ended).map_err(|error| Self::queued(build, error))?;
        io.stage(&StageLine::new(
            Verb::Built,
            format!(
                "deploy {} is live: {}, {}",
                Short(row.id),
                Plural(Self::count(row.files), "file"),
                Size(row.bytes.count())
            ),
        ))?;
        io.result(&Answer::of(row.id))?;
        io.json(&counts(Follow::document(Pushed::PUSH, site, &ended, Some(&row))))?;
        Ok(Exit::Done)
    }

    /// Sends the tree, and the files the site lacks, until it queues a build. The build, and how
    /// many files this run sent.
    fn send(&self, calls: &Calls, scanned: &Scanned, io: &mut Io<'_>) -> Result<(BuildId, u64), Error> {
        let mut sent = 0_u64;
        for _ in 0..self.rounds.max(1) {
            let probed = calls.block(calls.api.probe(scanned.tree.bytes()))?;
            match probed {
                Probed::Queued { build, .. } => return Ok((build, sent)),
                Probed::Duplicate => {
                    let overview = calls.block(calls.api.overview())?;
                    let queued = overview.pending.first().or(overview.build.as_ref());
                    let build =
                        queued.map(|build| build.id).ok_or(Failure::Incomplete { missing: 0, rounds: self.rounds })?;
                    return Ok((build, sent));
                }
                Probed::Missing { missing, part, .. } => {
                    let round = Round {
                        calls,
                        scanned,
                        limit: self.batch.map_or(part, |most| most.min(part)),
                        most: self.batch_files.max(1),
                        before: sent,
                    };
                    sent += Self::upload(&round, &missing, io)?;
                }
            }
        }
        let missing = match calls.block(calls.api.probe(scanned.tree.bytes()))? {
            Probed::Missing { missing, .. } => missing.len(),
            Probed::Queued { build, .. } => return Ok((build, sent)),
            Probed::Duplicate => 0,
        };
        Err(Failure::Incomplete { missing, rounds: self.rounds }.into())
    }

    /// Sends `missing`, a batch at a time. `before` is how many this run sent already, so a
    /// failure says how many the server holds.
    fn upload(round: &Round<'_>, missing: &[Hash], io: &mut Io<'_>) -> Result<u64, Error> {
        let Round { calls, scanned, limit, most, before } = *round;
        let batches = scanned.batches(missing, (limit, most))?;
        let total = missing.len() as u64;
        io.begin(Phase::Uploading, total, "files");
        let mut done = 0_u64;
        for (files, archive) in batches {
            let stored = calls.block(calls.api.parts(archive));
            if let Err(source) = stored {
                io.end()?;
                return Err(Failure::Uploading { sent: before + done, source: Box::new(source) }.into());
            }
            done += files;
            io.tick(done, None)?;
        }
        io.end()?;
        io.stage(&StageLine::new(Verb::Sent, Plural(Self::count(done), "file").to_string()))?;
        Ok(done)
    }
}
