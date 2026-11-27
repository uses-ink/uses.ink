//! One walk over the history behind a commit, giving every file its dates (decision 0036).

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};

use gix::ObjectId;
use gix::diff::tree::State;
use gix::diff::tree::recorder::Change;
use gix::objs::TreeRefIter;
use gix::odb::pack::cache::lru::MemoryCappedHashmap;
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;
use model::{Ability, AnyCode, ContentPath, FileFacts, Gap, Known, Lack, Oid, Provenance, Timestamp, UnknownReason};

use super::{Id, Repository};
use crate::error::Refused;

/// Commits handed to a thread at a time.
const BLOCK: usize = 256;

/// The history behind a commit, to be walked once.
#[derive(Debug)]
pub(super) struct History<'a> {
    pub(super) repo: &'a gix::Repository,
    /// The commit the snapshot is taken at.
    pub(super) tip: ObjectId,
    pub(super) threads: NonZeroUsize,
}

/// Every file's dates, and what the history lacked.
#[derive(Debug)]
pub(super) struct Dates {
    pub(super) facts: Vec<FileFacts>,
    pub(super) lacks: Vec<Lack>,
}

/// One commit to diff against its parent.
#[derive(Debug)]
struct Job {
    commit: Stamp,
    tree: ObjectId,
    /// The parent's tree; `None` for a root commit, diffed against the empty tree.
    parent: Option<ObjectId>,
}

/// A commit, by id, with its author time.
#[derive(Clone, Debug)]
struct Stamp {
    id: Oid,
    /// Seconds since the epoch, as the commit's author line says.
    time: i64,
}

/// What a commit did to a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Touch {
    Modified,
    Added,
    /// Added with the blob another path lost in the same commit: a move, not followed.
    Moved,
}

/// The paths of interest one commit touched, by their index.
type Hits = Vec<(u32, Touch)>;

impl History<'_> {
    /// The dates of each of `paths`, in their order.
    pub(super) fn dates(&self, paths: &[&ContentPath]) -> Result<Dates, Refused> {
        let index: HashMap<&[u8], u32> =
            paths.iter().zip(0u32..).map(|(path, at)| (path.as_str().as_bytes(), at)).collect();
        let (jobs, boundary) = self.jobs()?;
        let hits = self.diff(&jobs, &index)?;
        Ok(Fold { jobs: &jobs, hits: &hits, boundary: boundary.as_ref() }.dates(paths))
    }

    /// Walks the history once, newest commit time first, into one job per commit that is
    /// neither a merge nor a shallow boundary, and the newest boundary commit when there is one.
    fn jobs(&self) -> Result<(Vec<Job>, Option<Stamp>), Refused> {
        let git = |source| Refused::Git { source };
        // a repository that is not shallow has no boundary
        let shallow: HashSet<ObjectId> = match self.repo.shallow_commits().map_err(git)? {
            Some(commits) => commits.iter().copied().collect(),
            None => HashSet::new(),
        };
        let sorting = Sorting::ByCommitTime(CommitTimeOrder::default());
        let walk = self.repo.rev_walk([self.tip]).sorting(sorting).all().map_err(git)?;
        let mut trees: HashMap<ObjectId, ObjectId> = HashMap::new();
        let mut pending: Vec<(Stamp, ObjectId, ObjectId)> = Vec::new();
        let mut jobs = Vec::new();
        let mut boundary: Option<Stamp> = None;
        for info in walk {
            let info = info.map_err(git)?;
            let commit = info.object().map_err(git)?;
            let tree = commit.tree_id().map_err(git)?.detach();
            let time = commit.author().and_then(|author| author.time()).map_err(git)?.seconds;
            let stamp = Stamp { id: Id(info.id).try_into()?, time };
            trees.insert(info.id, tree);
            if shallow.contains(&info.id) {
                // its parents are cut off, so what it changed cannot be known
                if boundary.as_ref().is_none_or(|newest| time > newest.time) {
                    boundary = Some(stamp);
                }
                continue;
            }
            match info.parent_ids.as_slice() {
                [] => jobs.push(Job { commit: stamp, tree, parent: None }),
                [parent] => pending.push((stamp, tree, *parent)),
                _merge => {}
            }
        }
        for (commit, tree, parent) in pending {
            let parent = match trees.get(&parent) {
                Some(tree) => *tree,
                None => self.repo.find_commit(parent).and_then(|c| c.tree_id()).map_err(git)?.detach(),
            };
            jobs.push(Job { commit, tree, parent: Some(parent) });
        }
        Ok((jobs, boundary))
    }

    /// Diffs every job on [`History::threads`] threads, blocks of jobs handed out through a
    /// counter so a slow stretch of history does not hold up one thread.
    fn diff(&self, jobs: &[Job], index: &HashMap<&[u8], u32>) -> Result<Vec<Hits>, Refused> {
        let sync = self.repo.clone().into_sync();
        let next = AtomicUsize::new(0);
        let parts: Vec<Result<Vec<(usize, Hits)>, Refused>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..self.threads.get())
                .map(|_| {
                    let (sync, next) = (&sync, &next);
                    scope.spawn(move || {
                        let mut repo = sync.to_thread_local();
                        // a handle made from the shared one has no delta-base cache of its own
                        repo.objects.set_pack_cache(|| Box::new(MemoryCappedHashmap::new(Repository::DELTAS)));
                        let mut differ = Differ { repo, state: State::default(), index };
                        let mut done = Vec::new();
                        loop {
                            let from = next.fetch_add(BLOCK, Ordering::Relaxed);
                            if from >= jobs.len() {
                                return Ok(done);
                            }
                            for (at, job) in jobs.iter().enumerate().skip(from).take(BLOCK) {
                                done.push((at, differ.touched(job)?));
                            }
                        }
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
                .collect()
        });
        let mut hits: Vec<Hits> = vec![Vec::new(); jobs.len()];
        for part in parts {
            for (at, touched) in part? {
                if let Some(slot) = hits.get_mut(at) {
                    *slot = touched;
                }
            }
        }
        Ok(hits)
    }
}

/// One thread's diffing.
struct Differ<'a> {
    repo: gix::Repository,
    state: State,
    index: &'a HashMap<&'a [u8], u32>,
}

impl Differ<'_> {
    /// The paths of interest `job`'s commit touched. A recorder is made per diff: a reused one
    /// silently reports almost nothing (spikes/gix/README.md, Gotchas).
    fn touched(&mut self, job: &Job) -> Result<Hits, Refused> {
        let git = |source| Refused::Git { source };
        let new = self.repo.find_object(job.tree).map_err(git)?.detach();
        let old = match job.parent {
            Some(id) => self.repo.find_object(id).map_err(git)?.detach().data,
            None => Vec::new(),
        };
        let kind = self.repo.object_hash();
        let mut recorder = gix::diff::tree::Recorder::default();
        gix::diff::tree(
            TreeRefIter::from_bytes(&old, kind),
            TreeRefIter::from_bytes(&new.data, kind),
            &mut self.state,
            &self.repo.objects,
            &mut recorder,
        )
        .map_err(|source| Refused::Diff { source })?;
        Ok(self.hits(recorder.records))
    }

    /// The paths of interest among `changes`, a path added with a blob another path lost being
    /// a move.
    fn hits(&self, changes: Vec<Change>) -> Hits {
        let mut touched = Vec::new();
        let mut lost = HashSet::new();
        for change in changes {
            let (path, touch, mode, id) = match change {
                Change::Addition { path, entry_mode, oid, .. } => (path, Touch::Added, entry_mode, oid),
                Change::Modification { path, entry_mode, oid, .. } => (path, Touch::Modified, entry_mode, oid),
                Change::Deletion { entry_mode, oid, .. } => {
                    if !entry_mode.is_tree() {
                        lost.insert(oid);
                    }
                    continue;
                }
            };
            if mode.is_tree() {
                continue;
            }
            if let Some(&at) = self.index.get(path.as_slice()) {
                touched.push((at, touch, id));
            }
        }
        touched
            .into_iter()
            .map(|(at, touch, id)| match touch {
                Touch::Added if lost.contains(&id) => (at, Touch::Moved),
                touch => (at, touch),
            })
            .collect()
    }
}

impl std::fmt::Debug for Differ<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Differ").field("paths", &self.index.len()).finish_non_exhaustive()
    }
}

/// The diffs folded into dates, newest author time first: a rebase leaves author times out of
/// the order commits were made in, and the author time is when the change was written.
#[derive(Debug)]
struct Fold<'a> {
    jobs: &'a [Job],
    hits: &'a [Hits],
    boundary: Option<&'a Stamp>,
}

impl Fold<'_> {
    fn dates(&self, paths: &[&ContentPath]) -> Dates {
        let mut order: Vec<usize> = (0..self.jobs.len()).collect();
        order.sort_by_key(|&at| (Reverse(self.jobs.get(at).map_or(i64::MIN, |job| job.commit.time)), at));
        let mut created: Vec<Option<(&Stamp, Touch)>> = vec![None; paths.len()];
        let mut updated: Vec<Option<&Stamp>> = vec![None; paths.len()];
        for at in order {
            let (Some(job), Some(hits)) = (self.jobs.get(at), self.hits.get(at)) else { continue };
            for &(path, touch) in hits {
                let path = path as usize;
                if let Some(slot @ None) = updated.get_mut(path) {
                    *slot = Some(&job.commit);
                }
                if touch != Touch::Modified
                    && let Some(slot @ None) = created.get_mut(path)
                {
                    *slot = Some((&job.commit, touch));
                }
            }
        }
        let moved = created.iter().filter(|known| matches!(known, Some((_, Touch::Moved)))).count();
        let facts: Vec<FileFacts> = paths
            .iter()
            .zip(created.iter().zip(&updated))
            .map(|(path, (created, updated))| {
                let created = self.known(created.map(|(stamp, _)| stamp));
                let updated = self.known(*updated);
                FileFacts::new((*path).clone(), created, updated)
            })
            .collect();
        let lacks = self.lacks(&facts, moved as u64);
        Dates { facts, lacks }
    }

    /// What the history lacked: a boundary, when some fact is only bounded by it, and the
    /// renames it did not follow, when some file was created by one.
    fn lacks(&self, facts: &[FileFacts], moved: u64) -> Vec<Lack> {
        let mut lacks = Vec::new();
        if let Some(boundary) = self.boundary
            && let Ok(bound) = Timestamp::from_second(boundary.time)
        {
            let gap = Gap::Shallow { commit: boundary.id.clone(), bound };
            let bounded = |pick: fn(&FileFacts) -> &Known<Timestamp>| {
                facts.iter().any(|facts| matches!(pick(facts), Known::NotAfter { .. }))
            };
            if bounded(|facts| &facts.created) {
                lacks.push(Lack::new(Ability::History, AnyCode::Date, gap.clone()));
            }
            if bounded(|facts| &facts.updated) {
                lacks.push(Lack::new(Ability::History, AnyCode::Updated, gap));
            }
        }
        if moved > 0 {
            lacks.push(Lack::new(Ability::History, AnyCode::Date, Gap::Renames { moved }));
        }
        lacks
    }

    /// What is known from the commit that `stamp` names: its time; otherwise a bound at the
    /// shallow boundary, or nothing, since only a merge could have made the change.
    fn known(&self, stamp: Option<&Stamp>) -> Known<Timestamp> {
        let unknown = |reason| Known::Unknown { reason };
        match (stamp, self.boundary) {
            (Some(stamp), _) => match Timestamp::from_second(stamp.time) {
                Ok(value) => Known::Exact { value, from: Provenance::Commit { commit: stamp.id.clone() } },
                Err(_) => unknown(UnknownReason::NotReported),
            },
            (None, Some(boundary)) => match Timestamp::from_second(boundary.time) {
                Ok(value) => Known::NotAfter { value, from: Provenance::GitHistory { commit: boundary.id.clone() } },
                Err(_) => unknown(UnknownReason::NotReported),
            },
            (None, None) => unknown(UnknownReason::Merged),
        }
    }
}
