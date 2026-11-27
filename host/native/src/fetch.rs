//! The `git` capability on a local disk (decisions 0036 and 0046): a repository fetched into a
//! cache folder the host manages, then read at one commit through [`Repository`].
//!
//! [`Fetch`] is the operator's side of it. What the builder lets a source reach is its
//! [`Network`] setting, which defaults to off; what a clone may cost is [`FetchLimits`]; which
//! credentials the builder holds is [`Grants`], and a request only ever names one. gix is opened
//! isolated, with no user or system configuration, no credential helper and no prompt, so
//! nothing outside these settings changes what a fetch does. A clone is bare, in
//! `<cache>/git/<key>` where the key is [`Request::key`], and a second fetch of the same request
//! brings it up to date and to the depth asked for.
//!
//! The byte budget is watched while the pack arrives, so a remote that sends without end is cut
//! at the limit; the object budget is counted once the objects are indexed. A clone that passes a
//! budget is removed. Only `file://` has a transport so far: an `https://` repository is the
//! typed refusal [`Refused::Transport`] until an HTTP transport is chosen.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gix::remote::Direction;
use gix::remote::fetch::Shallow;
use model::git::{Auth, Budget, Cloned, Depth, Git, Grant, Network, Refused, Request, Scheme};
use model::{Bytes, Hash, Oid, Snapshot};

use crate::blob::Store;
use crate::budget::Budget as Tree;
use crate::error::Refused as Unread;
use crate::git::{Limits as History, Repository, Revision};
use crate::read::Read;

/// What a clone may cost: an operator's limit that a site may only lower (decision 0044).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchLimits {
    /// The most the clone may take on disk: `source.budget.bytes`.
    pub bytes: Bytes,
    /// The most objects it may hold: `source.budget.objects`.
    pub objects: u64,
}

impl Default for FetchLimits {
    /// 1 GiB, as architecture.md section 13 says, and five million objects, which is more than
    /// the Linux kernel's history holds.
    fn default() -> Self {
        Self { bytes: Bytes(1 << 30), objects: 5_000_000 }
    }
}

/// The credentials the builder holds, by the name a request gives them. A secret redacts itself
/// in `Debug` and has no `Display`; a plugin only ever names one.
#[derive(Clone, Debug, Default)]
pub struct Grants(BTreeMap<Grant, Secret>);

/// A user name and password, or a token as the password.
#[derive(Clone)]
struct Secret {
    user: String,
    password: String,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Secret(..)")
    }
}

impl Grants {
    /// Holds `password` for `user` under `grant`.
    pub fn hold(&mut self, grant: Grant, user: impl Into<String>, password: impl Into<String>) {
        self.0.insert(grant, Secret { user: user.into(), password: password.into() });
    }

    fn secret(&self, grant: &Grant) -> Option<&Secret> {
        self.0.get(grant)
    }
}

/// Why a fetch or a read failed with no case of its own.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failed {
    /// The cache folder could not be made, or a clone could not be removed or measured.
    Cache {
        /// The folder.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// git could not fetch: the remote is unreachable or sent something unusable.
    Fetch {
        /// What git said.
        source: gix::Error,
    },
    /// A clone the cache holds could not be opened.
    Clone {
        /// What git said.
        source: gix::Error,
    },
    /// The fetch ran past the wall-clock deadline of the call that asked for it.
    Late,
    /// The fetched commit's tree or history could not be read.
    Read(Unread),
}

impl std::fmt::Display for Failed {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cache { path, .. } => write!(out, "the clone cache {} could not be used", path.display()),
            Self::Fetch { .. } => out.write_str("the repository could not be fetched"),
            Self::Clone { .. } => out.write_str("a clone held in the cache could not be opened"),
            Self::Late => out.write_str("the fetch ran past the time of the call that asked for it"),
            Self::Read(_) => out.write_str("the fetched commit could not be read"),
        }
    }
}

impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cache { source, .. } => Some(source),
            Self::Fetch { source } | Self::Clone { source } => Some(source),
            Self::Read(source) => Some(source),
            Self::Late => None,
        }
    }
}

/// The host's fetching: the cache, what the builder allows, what a clone may cost, and the
/// credentials it holds.
#[derive(Clone, Debug)]
pub struct Fetch {
    /// The folder clones are kept in, under `git/`.
    pub cache: PathBuf,
    /// What the builder lets a source reach. Off until the operator says otherwise.
    pub network: Network,
    /// What a clone may cost.
    pub limits: FetchLimits,
    /// The credentials the builder holds.
    pub grants: Grants,
    /// How much of a commit's tree a snapshot may read.
    pub tree: Tree,
    /// What reading a clone's history spends.
    pub history: History,
}

impl Fetch {
    /// Fetching into `cache` with the network off, the default limits and no credential.
    pub fn new(cache: impl Into<PathBuf>) -> Self {
        Self {
            cache: cache.into(),
            network: Network::Off,
            limits: FetchLimits::default(),
            grants: Grants::default(),
            tree: Tree::default(),
            history: History::default(),
        }
    }

    /// Whether the repository at `scheme` has a transport in this build: only `file` so far.
    const fn reaches(scheme: Scheme) -> bool {
        matches!(scheme, Scheme::File)
    }

    /// The refusals that come before anything is fetched.
    fn allow(&self, request: &Request) -> Result<(), Refused<Failed>> {
        let scheme = request.locator.scheme();
        if !self.network.allows(scheme) {
            return Err(Refused::Network { scheme, policy: self.network });
        }
        if let Some(grant) = request.grant.as_ref().filter(|grant| self.grants.secret(grant).is_none()) {
            return Err(Refused::Auth { grant: grant.clone(), outcome: Auth::Absent });
        }
        if !Self::reaches(scheme) {
            return Err(Refused::Transport { scheme });
        }
        Ok(())
    }

    /// The folder the clone of `request` is kept in.
    fn folder(&self, key: &Hash) -> PathBuf {
        self.cache.join("git").join(key.to_string())
    }

    /// Fetches the repository, or brings a clone already held to the depth asked for, and says
    /// which commit the revision names.
    pub fn run(&self, request: &Request) -> Result<Cloned, Refused<Failed>> {
        self.run_until(request, None)
    }

    /// Like [`Fetch::run`], cut at `deadline` when one is given: a fetch still running then is
    /// interrupted, its clone removed, and [`Failed::Late`] is the refusal.
    pub fn run_until(&self, request: &Request, deadline: Option<Instant>) -> Result<Cloned, Refused<Failed>> {
        self.allow(request)?;
        let key = request.key();
        let folder = self.folder(&key);
        let held = folder.join("HEAD").is_file();
        if let Some(parent) = folder.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| Refused::Failed(Failed::Cache { path: parent.to_owned(), source }))?;
        }
        let watch = Watch { folder: &folder, limit: self.limits.bytes, deadline };
        let repo = watch.over(|interrupt| {
            if held { self.update(request, &folder, interrupt) } else { self.clone(request, &folder, interrupt) }
        })?;
        let counted =
            repo.objects.iter().map(Iterator::count).map_err(|source| Refused::Failed(Failed::Clone { source }))?;
        let objects = u64::try_from(counted).unwrap_or(u64::MAX);
        if objects > self.limits.objects {
            watch.remove()?;
            return Err(Refused::Budget(Budget::Objects { count: objects, limit: self.limits.objects }));
        }
        let shallow = repo.is_shallow().map_err(|source| Refused::Failed(Failed::Clone { source }))?;
        let commit = Self::commit(request, &folder)?;
        let bytes = Footprint(&folder)
            .bytes()
            .map_err(|source| Refused::Failed(Failed::Cache { path: folder.clone(), source }))?;
        Ok(Cloned { clone: key, commit, shallow, objects, bytes })
    }

    /// The commit the request's revision names in the clone at `folder`.
    fn commit(request: &Request, folder: &Path) -> Result<Oid, Refused<Failed>> {
        let revision = request.revision.as_ref().map_or(Revision::Head, |asked| Revision::Spec(asked.to_string()));
        let at = Repository { revision, ..Repository::new(folder) };
        at.tip().map_err(|refused| match (refused, &request.revision) {
            (Unread::Revision { .. }, Some(asked)) => Refused::Revision { written: asked.clone() },
            (other, _) => Refused::Failed(Failed::Read(other)),
        })
    }

    /// The shallow setting that gets a fresh clone to `depth`.
    fn shallow(depth: Depth) -> Shallow {
        match depth {
            Depth::Commits(count) => Shallow::DepthAtRemote(count),
            _ => Shallow::NoChange,
        }
    }

    /// The shallow setting that gets a clone already held to `depth`: a full request undoes the
    /// boundary, and a bounded one never cuts history already there.
    fn deepen(depth: Depth, held: bool) -> Shallow {
        match (depth, held) {
            (Depth::Commits(count), _) => Shallow::DepthAtRemote(count),
            (_, true) => Shallow::undo(),
            (_, false) => Shallow::NoChange,
        }
    }

    /// The credentials of the request's grant, as gix's callback answers for them.
    fn credentials(&self, request: &Request) -> Option<Secret> {
        request.grant.as_ref().and_then(|grant| self.grants.secret(grant)).cloned()
    }

    fn clone(&self, request: &Request, folder: &Path, interrupt: &AtomicBool) -> Result<gix::Repository, gix::Error> {
        let written = request.locator.to_string();
        let url = gix::url::parse(gix::bstr::BStr::new(&written))?;
        let (create, open) = (gix::create::Options::default(), gix::open::Options::isolated());
        let mut prepared = gix::clone::PrepareFetch::new(url, folder, gix::create::Kind::Bare, create, open)?
            .with_shallow(Self::shallow(request.depth));
        if let Some(asked) = &request.revision {
            prepared = prepared.with_ref_name(Some(asked.as_str()))?;
        }
        if let Some(secret) = self.credentials(request) {
            prepared = prepared.configure_connection(move |connection| {
                connection.set_credentials(secret.answer());
                Ok(())
            });
        }
        let (repo, _outcome) = prepared.fetch_only(gix::progress::Discard, interrupt)?;
        Ok(repo)
    }

    fn update(&self, request: &Request, folder: &Path, interrupt: &AtomicBool) -> Result<gix::Repository, gix::Error> {
        let mut repo = gix::open_opts(folder, gix::open::Options::isolated())?;
        let shallow = Self::deepen(request.depth, repo.is_shallow()?);
        let remote =
            repo.find_fetch_remote(None)?.with_refspecs(Some("+refs/heads/*:refs/heads/*"), Direction::Fetch)?;
        let mut connection = remote.connect(Direction::Fetch)?;
        if let Some(secret) = self.credentials(request) {
            connection.set_credentials(secret.answer());
        }
        let prepared = connection.prepare_fetch(gix::progress::Discard, gix::remote::ref_map::Options::default())?;
        prepared.with_shallow(shallow).receive(gix::progress::Discard, interrupt)?;
        repo.object_cache_size(0);
        Ok(repo)
    }

    /// The snapshot of `cloned` at its commit, its files stored in `store`.
    pub fn read(&self, cloned: &Cloned, store: &mut impl Store) -> Result<Read, Refused<Failed>> {
        let folder = self.folder(&cloned.clone);
        let at = Repository {
            revision: Revision::Spec(cloned.commit.to_string()),
            budget: self.tree,
            limits: self.history,
            ..Repository::new(folder)
        };
        at.read(store).map_err(|refused| Refused::Failed(Failed::Read(refused)))
    }
}

impl Secret {
    /// gix's credential callback answering every request with this secret and storing nothing.
    fn answer(
        &self,
    ) -> impl FnMut(gix::credentials::helper::Action) -> gix::Result<Option<gix::credentials::protocol::Outcome>> + use<>
    {
        let this = self.clone();
        move |action| {
            Ok(match action {
                gix::credentials::helper::Action::Get(context) => Some(gix::credentials::protocol::Outcome {
                    credentials: gix::sec::identity::Credentials::UsernamePassword {
                        username: this.user.clone(),
                        password: this.password.clone(),
                        oauth_refresh_token: None,
                    },
                    next: context.into(),
                }),
                _ => None,
            })
        }
    }
}

/// Watches a clone's folder while gix fills it, and interrupts the fetch past the byte limit or
/// the deadline.
struct Watch<'f> {
    folder: &'f Path,
    limit: Bytes,
    deadline: Option<Instant>,
}

impl Watch<'_> {
    /// How often the folder is measured.
    const EVERY: Duration = Duration::from_millis(10);

    /// Runs `fetch` with a watcher beside it; a fetch the watcher cut, or that ended past the
    /// limit, removes the clone and is the budget's refusal.
    fn over<T>(&self, fetch: impl FnOnce(&AtomicBool) -> Result<T, gix::Error>) -> Result<T, Refused<Failed>> {
        let (interrupt, done, late) = (AtomicBool::new(false), AtomicBool::new(false), AtomicBool::new(false));
        let result = std::thread::scope(|scope| {
            scope.spawn(|| {
                while !done.load(Ordering::Relaxed) {
                    if self.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        late.store(true, Ordering::Relaxed);
                        interrupt.store(true, Ordering::Relaxed);
                    }
                    if Footprint(self.folder).bytes().is_ok_and(|size| size > self.limit) {
                        interrupt.store(true, Ordering::Relaxed);
                    }
                    std::thread::sleep(Self::EVERY);
                }
            });
            let result = fetch(&interrupt);
            done.store(true, Ordering::Relaxed);
            result
        });
        if late.load(Ordering::Relaxed) {
            self.remove()?;
            return Err(Refused::Failed(Failed::Late));
        }
        // an unmeasured clone is not taken as empty: the budget cannot pass what it cannot count
        let size = Footprint(self.folder)
            .bytes()
            .map_err(|source| Refused::Failed(Failed::Cache { path: self.folder.to_owned(), source }))?;
        if interrupt.load(Ordering::Relaxed) || (result.is_ok() && size > self.limit) {
            self.remove()?;
            return Err(Refused::Budget(Budget::Bytes { size, limit: self.limit }));
        }
        result.map_err(|source| Refused::Failed(Failed::Fetch { source }))
    }

    /// Removes the clone: the host made the folder, and a clone past a budget is not kept.
    fn remove(&self) -> Result<(), Refused<Failed>> {
        match std::fs::remove_dir_all(self.folder) {
            Err(source) if source.kind() != std::io::ErrorKind::NotFound => {
                Err(Refused::Failed(Failed::Cache { path: self.folder.to_owned(), source }))
            }
            _ => Ok(()),
        }
    }
}

/// How much disk a folder's files take.
struct Footprint<'f>(&'f Path);

impl Footprint<'_> {
    fn bytes(&self) -> std::io::Result<Bytes> {
        let mut total = 0_u64;
        let mut pending = vec![self.0.to_owned()];
        while let Some(folder) = pending.pop() {
            let entries = match std::fs::read_dir(&folder) {
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
                other => other?,
            };
            for entry in entries {
                let entry = entry?;
                let kind = entry.file_type()?;
                if kind.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                match entry.metadata() {
                    Ok(metadata) => total = total.saturating_add(metadata.len()),
                    // gone since it was listed, so it takes no disk; any other failure is the
                    // budget's to know, since an unmeasured file could be any size
                    Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                    Err(source) => return Err(source),
                }
            }
        }
        Ok(Bytes(total))
    }
}

/// The capability as a native trait: fetching through a [`Fetch`], and storing each snapshot's
/// files in a store the caller keeps. The files of a snapshot are read once, by the last
/// [`Git::snapshot`] call.
#[derive(Debug)]
pub struct Cloning<'a, S> {
    fetch: &'a Fetch,
    store: &'a mut S,
    read: Option<Read>,
}

impl<'a, S: Store> Cloning<'a, S> {
    /// Fetches through `fetch`, storing files in `store`.
    pub fn new(fetch: &'a Fetch, store: &'a mut S) -> Self {
        Self { fetch, store, read: None }
    }

    /// What the last snapshot's read left out and found worth saying: folders skipped, LFS pointers.
    pub fn take(&mut self) -> Option<Read> {
        self.read.take()
    }
}

impl<S: Store> Git for Cloning<'_, S> {
    type Error = Failed;

    fn fetch(&mut self, request: &Request) -> Result<Cloned, Refused<Failed>> {
        self.fetch.run(request)
    }

    fn snapshot(&mut self, cloned: &Cloned) -> Result<Snapshot, Refused<Failed>> {
        let read = self.fetch.read(cloned, self.store)?;
        let snapshot = read.snapshot.clone();
        self.read = Some(read);
        Ok(snapshot)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_fetch_that_stalls_is_cut_at_the_deadline() {
        let folder = Path::new("a-clone-that-is-never-made");
        let watch = Watch { folder, limit: Bytes(1 << 30), deadline: Some(Instant::now() + Duration::from_millis(50)) };
        let started = Instant::now();
        let stalled = watch.over::<()>(|interrupt| {
            while !interrupt.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(gix::url::parse(gix::bstr::BStr::new("")).unwrap_err())
        });
        assert!(matches!(stalled, Err(Refused::Failed(Failed::Late))), "{stalled:?}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
