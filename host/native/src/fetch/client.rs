use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use gix::remote::Direction;
use gix::remote::fetch::Shallow;
use https::Https;
use model::git::{Auth, Budget, Cloned, Depth, Network, Refused, Request, Scheme};
use model::{Hash, Oid};

use super::grants::Secret;
use super::watch::{Footprint, Watch};
use super::{Failed, FetchLimits, Grants};
use crate::blob::Store;
use crate::budget::Budget as Tree;
use crate::error::Refused as Unread;
use crate::git::{Limits as History, Repository, Revision};
use crate::read::Read;

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
    /// How an https remote is reached: timeouts, redirects and extra trusted roots.
    pub https: Https,
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
            https: Https::default(),
            tree: Tree::default(),
            history: History::default(),
        }
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
        let secret = self.credentials(request);
        let https = (request.locator.scheme() == Scheme::Https).then(|| self.https.clone());
        prepared = prepared.configure_connection(move |connection| {
            if let Some(https) = &https {
                connection.set_transport_options(https.options().map_err(gix::Error::from_error)?);
            }
            if let Some(secret) = &secret {
                connection.set_credentials(secret.answer());
            }
            Ok(())
        });
        let (repo, _outcome) = prepared.fetch_only(gix::progress::Discard, interrupt)?;
        Ok(repo)
    }

    fn update(&self, request: &Request, folder: &Path, interrupt: &AtomicBool) -> Result<gix::Repository, gix::Error> {
        let mut repo = gix::open_opts(folder, gix::open::Options::isolated())?;
        let shallow = Self::deepen(request.depth, repo.is_shallow()?);
        let remote =
            repo.find_fetch_remote(None)?.with_refspecs(Some("+refs/heads/*:refs/heads/*"), Direction::Fetch)?;
        let mut connection = remote.connect(Direction::Fetch)?;
        if request.locator.scheme() == Scheme::Https {
            connection.set_transport_options(self.https.options().map_err(gix::Error::from_error)?);
        }
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
