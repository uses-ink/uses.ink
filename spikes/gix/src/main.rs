//! M0 spike 2: gix shallow clone, deepen, and per-file first and last commit dates.
//!
//! Usage:
//!   spike-gix clone  <url> <dir> [depth]      bare clone, depth 1 by default, 0 for full
//!   spike-gix deepen <dir> <n|full>           extend the shallow boundary
//!   spike-gix dates  <dir> [--first-parent] [--threads N]
//!   spike-gix all    <url> <dir> <n|full|0> [dates flags]
//!
//! Dates are the author time of the newest commit that touched a path (updated) and of the
//! newest commit that added it (created), for every blob at HEAD. History is walked once; each
//! commit is diffed against its parent once, in parallel, and the per-commit hits are folded in
//! commit order.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use gix::bstr::BString;
use gix::diff::tree::recorder::Change;
use gix::hash::ObjectId;
use gix::objs::TreeRefIter;
use gix::remote::fetch::Shallow;

/// Jobs handed to a thread at a time.
const BLOCK: usize = 256;

/// HEAD paths one commit touched.
type Hits = Vec<(u32, Touch)>;

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags = Flags::parse(&args);
    match args.first().map(String::as_str) {
        Some("clone") => {
            let depth = args.get(3).map_or(Ok(1), |d| d.parse())?;
            clone(&args[1], &args[2], depth)?;
        }
        Some("deepen") => {
            deepen(&args[1], &args[2])?;
        }
        Some("dates") => {
            dates(&args[1], &flags)?;
        }
        Some("all") => {
            let t = Instant::now();
            clone(&args[1], &args[2], 1)?;
            if args[3] != "0" {
                deepen(&args[2], &args[3])?;
            }
            dates(&args[2], &flags)?;
            println!("total: {:.2?}", t.elapsed());
        }
        _ => {
            eprintln!("usage: spike-gix clone|deepen|dates|all ...");
            std::process::exit(2);
        }
    }
    println!("peak rss: {} MiB", peak_rss_kib() / 1024);
    Ok(())
}

struct Flags {
    first_parent: bool,
    threads: usize,
}

impl Flags {
    fn parse(args: &[String]) -> Self {
        let threads = args
            .iter()
            .position(|a| a == "--threads")
            .and_then(|i| args.get(i + 1))
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
        Flags { first_parent: args.iter().any(|a| a == "--first-parent"), threads }
    }
}

fn clone(url: &str, dir: &str, depth: u32) -> Res<()> {
    let t = Instant::now();
    let interrupt = AtomicBool::new(false);
    let shallow = NonZeroU32::new(depth).map_or(Shallow::NoChange, Shallow::DepthAtRemote);
    let (repo, _) =
        gix::prepare_clone_bare(url, dir)?.with_shallow(shallow).fetch_only(gix::progress::Discard, &interrupt)?;
    println!("clone depth {depth}: {:.2?}, shallow={}, head={}", t.elapsed(), repo.is_shallow()?, repo.head_id()?);
    Ok(())
}

fn deepen(dir: &str, by: &str) -> Res<()> {
    let t = Instant::now();
    let repo = gix::open(dir)?;
    let shallow = match by {
        "full" => Shallow::undo(),
        n => Shallow::Deepen(n.parse()?),
    };
    // A bare clone persists no fetch refspec (same as git), so name the one branch again.
    let branch = repo.head_name()?.ok_or("detached HEAD")?;
    let spec = format!("+{0}:{0}", branch.as_bstr());
    let remote = repo
        .find_default_remote(gix::remote::Direction::Fetch)
        .ok_or("no remote")??
        .with_refspecs([spec.as_str()], gix::remote::Direction::Fetch)?;
    let interrupt = AtomicBool::new(false);
    remote
        .connect(gix::remote::Direction::Fetch)?
        .prepare_fetch(gix::progress::Discard, Default::default())?
        .with_shallow(shallow)
        .receive(gix::progress::Discard, &interrupt)?;
    let repo = gix::open(dir)?;
    let boundary = repo.shallow_commits()?.map_or(0, |c| c.len());
    println!("deepen {by}: {:.2?}, shallow={}, boundary commits={boundary}", t.elapsed(), repo.is_shallow()?);
    Ok(())
}

/// One commit to diff: its tree, the tree it is diffed against (None means the empty tree,
/// for a root commit), and its author time.
struct Job {
    tree: ObjectId,
    parent_tree: Option<ObjectId>,
    time: i64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Touch {
    Modified,
    Added,
    /// Added with the same blob that a deleted path had in the same commit.
    Moved,
}

#[derive(Clone, Copy, Debug)]
#[expect(dead_code, reason = "values are read through Debug in SPIKE_DUMP")]
enum Fact {
    Exact(i64),
    NotAfter(i64),
    Unknown,
}

fn dates(dir: &str, flags: &Flags) -> Res<()> {
    let t_all = Instant::now();
    // core.deltaBaseCacheLimit sizes the dynamic pack (delta base) cache of every handle.
    let pack_cache = format!("core.deltaBaseCacheLimit={}m", env_usize("SPIKE_PACK_CACHE_MB", 32));
    let mut repo = gix::open_opts(dir, gix::open::Options::default().config_overrides([pack_cache]))?;
    repo.object_cache_size(64 * 1024 * 1024);

    // Files at HEAD.
    let t = Instant::now();
    let head = repo.head_commit()?;
    let files: Vec<BString> = head
        .tree()?
        .traverse()
        .breadthfirst
        .files()?
        .into_iter()
        .filter(|e| e.mode.is_blob_or_symlink())
        .map(|e| e.filepath)
        .collect();
    let index: HashMap<&[u8], u32> = files.iter().enumerate().map(|(i, p)| (p.as_slice(), i as u32)).collect();
    println!("head files: {} in {:.2?}", files.len(), t.elapsed());

    // Walk once, newest first, collecting diff jobs.
    let t = Instant::now();
    let shallow: HashSet<ObjectId> = repo.shallow_commits()?.map(|c| c.iter().copied().collect()).unwrap_or_default();
    let mut walk = repo.rev_walk([head.id]).sorting(gix::revision::walk::Sorting::ByCommitTime(Default::default()));
    if flags.first_parent {
        walk = walk.first_parent_only();
    }
    let mut jobs = Vec::new();
    let (mut commits, mut merges, mut boundary_time) = (0usize, 0usize, None::<i64>);
    let mut tree_of: HashMap<ObjectId, ObjectId> = HashMap::new();
    let mut pending: Vec<(ObjectId, ObjectId, i64)> = Vec::new(); // (tree, parent commit, time)
    for info in walk.all()? {
        let info = info?;
        commits += 1;
        let commit = info.object()?;
        let tree = commit.tree_id()?.detach();
        let time = commit.author()?.seconds();
        tree_of.insert(info.id, tree);
        if shallow.contains(&info.id) {
            // Parents cut by the shallow boundary: this commit's own changes are unknowable.
            boundary_time = Some(boundary_time.map_or(time, |b| b.max(time)));
            continue;
        }
        match info.parent_ids.as_slice() {
            [] => jobs.push(Job { tree, parent_tree: None, time }),
            [p] => pending.push((tree, *p, time)),
            [p, ..] if flags.first_parent => pending.push((tree, *p, time)),
            _ => merges += 1,
        }
    }
    // Parent trees: every parent is reachable, so it was walked too.
    for (tree, parent, time) in pending {
        let parent_tree = match tree_of.get(&parent) {
            Some(t) => *t,
            None => repo.find_commit(parent)?.tree_id()?.detach(),
        };
        jobs.push(Job { tree, parent_tree: Some(parent_tree), time });
    }
    println!("walk: {commits} commits ({merges} merges skipped, {} diffs) in {:.2?}", jobs.len(), t.elapsed());

    // Diff in parallel, in walk order so neighbouring commits share cached trees. Blocks of
    // jobs are handed out through a counter, so a slow stretch of history does not stall one
    // thread. Each job yields the HEAD paths it touched and whether it added them.
    let t = Instant::now();
    let sync = repo.clone().into_sync();
    let next = AtomicUsize::new(0);
    let obj_cache = env_usize("SPIKE_OBJ_CACHE_MB", 0) * 1024 * 1024;
    let mut hits: Vec<Hits> = vec![Vec::new(); jobs.len()];
    let parts: Vec<Vec<(usize, Hits)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..flags.threads.max(1))
            .map(|_| {
                let (sync, index, jobs, next) = (&sync, &index, &jobs, &next);
                s.spawn(move || -> Res<Vec<(usize, Hits)>> {
                    let mut repo = sync.to_thread_local();
                    repo.object_cache_size(obj_cache);
                    let mut state = gix::diff::tree::State::default();
                    let kind = repo.object_hash();
                    let mut out = Vec::new();
                    loop {
                        let from = next.fetch_add(BLOCK, Ordering::Relaxed);
                        if from >= jobs.len() {
                            break;
                        }
                        for (i, job) in jobs.iter().enumerate().skip(from).take(BLOCK) {
                            let new = repo.find_object(job.tree)?.detach();
                            let old = match job.parent_tree {
                                Some(id) => repo.find_object(id)?.detach().data,
                                None => Vec::new(),
                            };
                            let mut rec = gix::diff::tree::Recorder::default();
                            gix::diff::tree(
                                TreeRefIter::from_bytes(&old, kind),
                                TreeRefIter::from_bytes(&new.data, kind),
                                &mut state,
                                &repo.objects,
                                &mut rec,
                            )?;
                            // A HEAD path added with the exact blob another path lost in the
                            // same commit is a pure move. Counted, not followed.
                            let mut touched = Vec::new();
                            let mut deleted = HashSet::new();
                            for change in rec.records.drain(..) {
                                let (path, kind, mode, oid) = match change {
                                    Change::Addition { path, entry_mode, oid, .. } => {
                                        (path, Touch::Added, entry_mode, oid)
                                    }
                                    Change::Modification { path, entry_mode, oid, .. } => {
                                        (path, Touch::Modified, entry_mode, oid)
                                    }
                                    Change::Deletion { entry_mode, oid, .. } => {
                                        if !entry_mode.is_tree() {
                                            deleted.insert(oid);
                                        }
                                        continue;
                                    }
                                };
                                if mode.is_tree() {
                                    continue;
                                }
                                if let Some(&p) = index.get(path.as_slice()) {
                                    touched.push((p, kind, oid));
                                }
                            }
                            let touched = touched
                                .into_iter()
                                .map(|(p, kind, oid)| match kind {
                                    Touch::Added if deleted.contains(&oid) => (p, Touch::Moved),
                                    k => (p, k),
                                })
                                .collect();
                            out.push((i, touched));
                        }
                    }
                    Ok(out)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("thread panicked").expect("diff failed")).collect()
    });
    for (i, touched) in parts.into_iter().flatten() {
        hits[i] = touched;
    }
    println!("diff: {:.2?} on {} threads", t.elapsed(), flags.threads);

    // Fold order: newest author time first; the walk order is by commit time, which differs
    // after rebases.
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(jobs[i].time));

    // Fold newest first: first touch is `updated`, first addition is `created`.
    let mut created = vec![Fact::Unknown; files.len()];
    let mut updated = vec![Fact::Unknown; files.len()];
    let mut moved = 0usize;
    for &j in &order {
        let job = &jobs[j];
        for &(i, touch) in &hits[j] {
            let i = i as usize;
            if matches!(updated[i], Fact::Unknown) {
                updated[i] = Fact::Exact(job.time);
            }
            if touch != Touch::Modified && matches!(created[i], Fact::Unknown) {
                created[i] = Fact::Exact(job.time);
                moved += usize::from(touch == Touch::Moved);
            }
        }
    }
    // Whatever is left existed at the shallow boundary: a bound, never a date.
    if let Some(b) = boundary_time {
        for f in created.iter_mut().chain(updated.iter_mut()) {
            if matches!(f, Fact::Unknown) {
                *f = Fact::NotAfter(b);
            }
        }
    }
    println!("dates: {:.2?}", t_all.elapsed());
    report("created", &created);
    report("updated", &updated);
    println!("  created by a pure move (rename not followed): {moved}");
    if std::env::var_os("SPIKE_DUMP").is_some() {
        for (i, p) in files.iter().enumerate() {
            println!("{p}\t{:?}\t{:?}", created[i], updated[i]);
        }
    }
    Ok(())
}

fn report(name: &str, facts: &[Fact]) {
    let (mut exact, mut bound, mut unknown) = (0, 0, 0);
    for f in facts {
        match f {
            Fact::Exact(_) => exact += 1,
            Fact::NotAfter(_) => bound += 1,
            Fact::Unknown => unknown += 1,
        }
    }
    println!("  {name}: exact {exact}, not-after (beyond depth) {bound}, unknown {unknown}");
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn peak_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0)
}
