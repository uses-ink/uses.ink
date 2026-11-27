//! Source upload: the tree of a folder, the blobs it is missing, and the build of it (`ink push`,
//! decision 0076).
//!
//! Three calls on one path. `POST` takes the tree (`uses.tree/v1`: every file by path, hash, size
//! and time), and answers which of its blobs the site's tenant does not hold yet, or, when it holds
//! them all, stores the tree, queues a build of it and says which build. `PUT` takes a tar archive
//! of blobs, each named by its hash, checks every one against its name and stores it. `GET` gives
//! the files of the latest tree back as a tar archive. The server keeps no upload session: the
//! tenant's blobs are the state, so an upload that stopped is resumed by sending the tree again,
//! and a request holds in memory at most the operator's `part` limit.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use blob::{Blob, Chunks, Tenant};
use build::{BuildId, Tree, TreeError, Trigger};
use clock::{Clock, Draw};
use control::{NewJob, Ports, Requested, Site, SiteKind};
use model::{Bytes, ContentPath, Hash};
use serde::Serialize;

use crate::Api;
use crate::archive::{self, Writer};
use crate::auth::credential::{Credentials, Principal};
use crate::fault::Fault;
use crate::limits::Limits;
use crate::reply::{Media, Reply, Status};

/// Why a tree or a batch of blobs is not taken.
#[derive(Debug)]
#[non_exhaustive]
pub enum Sourced {
    /// The body is not a tree.
    Tree(TreeError),
    /// More files than the limit.
    Files {
        /// How many the tree lists.
        count: u64,
        /// The limit.
        limit: u64,
    },
    /// A file larger than the limit.
    File {
        /// The file.
        path: ContentPath,
        /// Its size.
        size: Bytes,
        /// The limit.
        limit: u64,
    },
    /// A file larger than one request may carry.
    Part {
        /// The file.
        path: ContentPath,
        /// Its size.
        size: Bytes,
        /// The limit of a request's files together.
        limit: u64,
    },
    /// All files together larger than the limit.
    Total {
        /// Their size.
        size: Bytes,
        /// The limit.
        limit: u64,
    },
    /// A path longer than the limit.
    Name {
        /// The path.
        path: ContentPath,
        /// The limit.
        limit: usize,
    },
    /// A blob of the tree is held with another size than the tree says.
    Size {
        /// The file.
        path: ContentPath,
        /// What the tree says.
        listed: Bytes,
        /// What is held.
        held: Bytes,
    },
    /// An entry of a batch whose name is no hash.
    Unnamed {
        /// The name.
        name: String,
    },
    /// An entry of a batch whose bytes are not the hash it is named for.
    Mismatch {
        /// The name.
        name: Hash,
        /// What the bytes hash to.
        found: Hash,
    },
    /// The site takes builds of a git repository, not uploads.
    NotUpload,
    /// No tree has been pushed to the site.
    NoSource,
    /// The tree names a blob the tenant does not hold: a blob removed since the tree was stored.
    Absent {
        /// The blob.
        blob: Hash,
    },
}

impl fmt::Display for Sourced {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tree(_) => out.write_str("the body is not a tree of files"),
            Self::Files { count, limit } => write!(out, "the tree lists {count} files, over the limit of {limit}"),
            Self::File { path, size, limit } => {
                write!(out, "{path} is {} bytes, over the limit of {limit} a file", size.count())
            }
            Self::Part { path, size, limit } => write!(
                out,
                "{path} is {} bytes, more than the {limit} one upload request carries: the operator raises `--upload-part`",
                size.count()
            ),
            Self::Total { size, limit } => {
                write!(out, "the files hold {} bytes, over the limit of {limit}", size.count())
            }
            Self::Name { path, limit } => write!(out, "the path {path} is longer than the limit of {limit} bytes"),
            Self::Size { path, listed, held } => {
                write!(out, "{path} is listed as {} bytes, and the blob held is {} bytes", listed.count(), held.count())
            }
            Self::Unnamed { name } => write!(out, "{name:?} is no hash: a batch names each blob by its hash"),
            Self::Mismatch { name, found } => write!(out, "the blob named {name} hashes to {found}"),
            Self::NotUpload => out.write_str("this site is built from a git repository: it takes no upload"),
            Self::NoSource => out.write_str("no tree has been pushed to this site yet"),
            Self::Absent { blob } => write!(out, "the tree lists {blob}, which is no longer held"),
        }
    }
}

impl Error for Sourced {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Tree(source) => Some(source),
            _ => None,
        }
    }
}

impl Sourced {
    /// The name the refusal is answered under.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Tree(_) => "source.tree.unreadable",
            Self::Files { .. } => "source.files.over",
            Self::File { .. } => "source.file.over",
            Self::Part { .. } => "source.part.over",
            Self::Total { .. } => "source.total.over",
            Self::Name { .. } => "source.name.over",
            Self::Size { .. } => "source.size.mismatch",
            Self::Unnamed { .. } => "source.blob.unnamed",
            Self::Mismatch { .. } => "source.blob.mismatch",
            Self::NotUpload => "source.site.not-upload",
            Self::NoSource => "source.tree.none",
            Self::Absent { .. } => "source.blob.absent",
        }
    }

    /// The status it is answered with.
    pub const fn status(&self) -> Status {
        match self {
            Self::Tree(_) => Status::BadRequest,
            Self::Files { .. } | Self::File { .. } | Self::Part { .. } | Self::Total { .. } | Self::Name { .. } => {
                Status::TooLarge
            }
            Self::NoSource => Status::NotFound,
            _ => Status::Unprocessable,
        }
    }
}

/// What a tree sent to `POST /v1/sites/{site}/source` came to.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Probed {
    /// Blobs are missing: send them, then send the tree again.
    Missing {
        /// The tree's hash.
        tree: Hash,
        /// The blobs the tenant does not hold, each once.
        missing: Vec<Hash>,
        /// How many bytes of files one `PUT` takes.
        part: u64,
    },
    /// Every blob is held: the tree is stored and a build of it is queued.
    Queued {
        /// The tree's hash.
        tree: Hash,
        /// The build.
        build: BuildId,
        /// How many files the tree lists.
        files: u64,
        /// How many bytes they hold.
        bytes: Bytes,
    },
    /// A build with this key is queued already.
    Duplicate,
}

/// What a batch sent to `PUT /v1/sites/{site}/source` came to.
#[derive(Debug, Serialize)]
pub struct Stored {
    /// How many blobs the batch held.
    pub blobs: u64,
    /// How many of them were new.
    pub created: u64,
    /// How many bytes it held.
    pub bytes: Bytes,
}

impl Limits {
    /// Whether `tree` is within these limits, and which limit it is over when not.
    pub(crate) fn admit(&self, tree: &Tree) -> Result<(), Sourced> {
        let count = tree.files().len() as u64;
        if count > self.files {
            return Err(Sourced::Files { count, limit: self.files });
        }
        for leaf in tree.files() {
            if leaf.path.as_str().len() > self.name {
                return Err(Sourced::Name { path: leaf.path.clone(), limit: self.name });
            }
            if leaf.size.count() > self.file {
                return Err(Sourced::File { path: leaf.path.clone(), size: leaf.size, limit: self.file });
            }
            if leaf.size.count() > self.part {
                return Err(Sourced::Part { path: leaf.path.clone(), size: leaf.size, limit: self.part });
            }
        }
        if tree.total().count() > self.archive {
            return Err(Sourced::Total { size: tree.total(), limit: self.archive });
        }
        Ok(())
    }

    /// What one batch of blobs may hold.
    pub(crate) const fn parts(&self) -> archive::Limits {
        archive::Limits { files: self.files, file: self.file, total: self.part, name: self.name }
    }
}

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw> Api<P, C, K, D> {
    /// The site's tenant, when the site takes uploads.
    async fn uploads(&self, principal: Principal, id: control::Id<SiteKind>) -> Result<(Site, Tenant), Fault<P>> {
        let site = self.owned(principal, id).await?;
        if site.source != build::Source::Upload {
            return Err(Fault::Source(Sourced::NotUpload));
        }
        let tenant = Tenant::try_from(site.id).map_err(control::Refusal::Tenant)?;
        Ok((site, tenant))
    }

    /// The bytes of a blob of the tenant.
    async fn held(&self, tenant: &Tenant, blob: Hash) -> Result<Vec<u8>, Fault<P>> {
        let read = self.control.ports().blob().get(tenant, blob).await.map_err(|error| Fault::Control(error.into()))?;
        let read = read.ok_or(Fault::Source(Sourced::Absent { blob }))?;
        read.all().await.map_err(|error| Fault::Control(error.into()))
    }

    /// `POST /v1/sites/{site}/source`: the blobs a tree is missing, or a build of it.
    pub(crate) async fn probe(
        &self,
        principal: Principal,
        id: control::Id<SiteKind>,
        body: &[u8],
    ) -> Result<Reply, Fault<P>> {
        let (site, tenant) = self.uploads(principal, id).await?;
        let tree = Tree::read(body).map_err(|source| Fault::Source(Sourced::Tree(source)))?;
        self.limits.admit(&tree).map_err(Fault::Source)?;
        let blobs = self.control.ports().blob();
        let mut missing = BTreeSet::new();
        for leaf in tree.files() {
            match blobs.stat(&tenant, leaf.hash).await.map_err(|error| Fault::Control(error.into()))? {
                Some(held) if held != leaf.size => {
                    let (path, listed) = (leaf.path.clone(), leaf.size);
                    return Err(Fault::Source(Sourced::Size { path, listed, held }));
                }
                Some(_) => {}
                None => {
                    missing.insert(leaf.hash);
                }
            }
        }
        let hash = tree.hash();
        if !missing.is_empty() {
            let probed = Probed::Missing { tree: hash, missing: missing.into_iter().collect(), part: self.limits.part };
            return Self::json(Status::Ok, &probed);
        }
        blobs.put(&tenant, hash, [tree.bytes()]).await.map_err(|error| Fault::Control(error.into()))?;
        let new = NewJob {
            id: BuildId::from_bytes(self.drawn()?),
            site: site.id,
            trigger: Trigger::Upload,
            dedupe: None,
            at: self.clock.now(),
        };
        match self.control.request_upload(new, hash).await? {
            Requested::Queued(build) => {
                let (files, bytes) = (tree.files().len() as u64, tree.total());
                Self::json(Status::Created, &Probed::Queued { tree: hash, build, files, bytes })
            }
            _ => Self::json(Status::Ok, &Probed::Duplicate),
        }
    }

    /// `PUT /v1/sites/{site}/source`: a batch of blobs, each checked against its name.
    pub(crate) async fn parts(
        &self,
        principal: Principal,
        id: control::Id<SiteKind>,
        body: &[u8],
    ) -> Result<Reply, Fault<P>> {
        let (_, tenant) = self.uploads(principal, id).await?;
        let batch = archive::Tree::read(body, &self.limits.parts()).map_err(Fault::Archive)?;
        let mut named = Vec::new();
        for (path, bytes) in batch.files() {
            let name: Hash = path
                .as_str()
                .parse()
                .map_err(|_| Fault::Source(Sourced::Unnamed { name: path.as_str().to_owned() }))?;
            let found = Hash::of(bytes);
            if found != name {
                return Err(Fault::Source(Sourced::Mismatch { name, found }));
            }
            named.push((name, bytes));
        }
        let blobs = self.control.ports().blob();
        let mut stored = Stored { blobs: 0, created: 0, bytes: Bytes(0) };
        for (name, bytes) in named {
            let put = blobs.put(&tenant, name, [bytes]).await.map_err(|error| Fault::Control(error.into()))?;
            stored.blobs += 1;
            stored.created += u64::from(put.created);
            stored.bytes = Bytes(stored.bytes.count().saturating_add(bytes.len() as u64));
        }
        Self::json(Status::Ok, &stored)
    }

    /// `GET /v1/sites/{site}/source`: the files of the latest tree, as a tar archive.
    pub(crate) async fn bundle(&self, principal: Principal, id: control::Id<SiteKind>) -> Result<Reply, Fault<P>> {
        let site = self.owned(principal, id).await?;
        let tenant = Tenant::try_from(site.id).map_err(control::Refusal::Tenant)?;
        let hash = self.control.latest_tree(id).await?.ok_or(Fault::Source(Sourced::NoSource))?;
        let tree =
            Tree::read(&self.held(&tenant, hash).await?).map_err(|source| Fault::Source(Sourced::Tree(source)))?;
        let mut writer = Writer::new(self.limits.export);
        for leaf in tree.files() {
            writer.file(leaf.path.as_str(), &self.held(&tenant, leaf.hash).await?).map_err(Fault::Archive)?;
        }
        Ok(Reply::new(Status::Ok, Media::Tar, writer.finish()))
    }
}
