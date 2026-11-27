//! What the API answers with, as typed values that serialize.

use std::collections::BTreeMap;

use build::{BuildId, Source, Trigger};
use control::{AccountKind, Deploy, Id, Job, JobState, Live, Receipt, Recorded, Site, SiteKind, SiteState};
use jiff::Timestamp;
use model::{Bytes, DeployId, ExternalUrl};
use serde::Serialize;

/// Where a site's content comes from.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SourceView {
    /// A git repository.
    Git {
        /// Where.
        url: String,
        /// The branch, tag or commit, when one is named.
        revision: Option<String>,
    },
    /// Folders sent to the API.
    Upload,
    /// A folder on the builder's disk.
    Folder,
}

impl From<&Source> for SourceView {
    fn from(source: &Source) -> Self {
        match source {
            Source::Git { url, revision } => {
                Self::Git { url: url.to_string(), revision: revision.as_ref().map(ToString::to_string) }
            }
            Source::Upload => Self::Upload,
            _ => Self::Folder,
        }
    }
}

/// One site.
#[derive(Debug, Serialize)]
pub struct SiteView {
    /// Its id.
    pub id: Id<SiteKind>,
    /// Its handle.
    pub handle: String,
    /// The address it is served at.
    pub url: Option<ExternalUrl>,
    /// Whether it takes changes.
    pub state: SiteState,
    /// When it was added.
    pub created: Timestamp,
    /// Where its content comes from.
    pub source: SourceView,
}

impl SiteView {
    /// The view of `site`, served at `url`.
    pub fn of(site: &Site, url: Option<ExternalUrl>) -> Self {
        Self {
            id: site.id,
            handle: site.handle.to_string(),
            url,
            state: site.state,
            created: site.created,
            source: SourceView::from(&site.source),
        }
    }
}

/// One deploy.
#[derive(Debug, Serialize)]
pub struct DeployView {
    /// Its id.
    pub id: DeployId,
    /// When it was recorded.
    pub created: Timestamp,
    /// The build that made it, when one did.
    pub build: Option<BuildId>,
    /// How many routes it serves.
    pub files: u64,
    /// How many bytes they hold.
    pub bytes: Bytes,
    /// Whether it is the one served now.
    pub live: bool,
}

impl From<&Deploy> for DeployView {
    fn from(deploy: &Deploy) -> Self {
        Self {
            id: deploy.id,
            created: deploy.created,
            build: deploy.build,
            files: deploy.files,
            bytes: deploy.bytes,
            live: deploy.is_live,
        }
    }
}

/// The deploy a site serves.
#[derive(Debug, Serialize)]
pub struct LiveView {
    /// The deploy.
    pub deploy: DeployId,
    /// What it replaced.
    pub previous: Option<DeployId>,
    /// Whether builds are held back after a rollback.
    pub paused: bool,
    /// Since when.
    pub since: Timestamp,
}

impl From<&Live> for LiveView {
    fn from(live: &Live) -> Self {
        Self { deploy: live.deploy, previous: live.previous, paused: live.paused, since: live.since }
    }
}

/// One build.
#[derive(Debug, Serialize)]
pub struct BuildView {
    /// Its id.
    pub id: BuildId,
    /// What set it off.
    pub trigger: Trigger,
    /// Where it is.
    pub state: JobState,
    /// When it was requested.
    pub requested: Timestamp,
    /// When it ended.
    pub finished: Option<Timestamp>,
    /// The deploy it made.
    pub deploy: Option<DeployId>,
}

impl From<&Job> for BuildView {
    fn from(job: &Job) -> Self {
        Self {
            id: job.id,
            trigger: job.trigger,
            state: job.state,
            requested: job.requested,
            finished: job.finished,
            deploy: job.deploy,
        }
    }
}

/// `GET /v1/sites/{site}`: what a person looks at first.
#[derive(Debug, Serialize)]
pub struct Status {
    /// The site.
    pub site: SiteView,
    /// What it serves, if anything.
    pub live: Option<LiveView>,
    /// The latest build, if there is one.
    pub build: Option<BuildView>,
}

/// A list.
#[derive(Debug, Serialize)]
pub struct Listed<T> {
    /// The rows, newest first (sites: oldest first).
    pub items: Vec<T>,
}

/// What recording a deploy did.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Published {
    /// The deploy is live now.
    Live,
    /// The deploy is recorded and publishing is paused, so it is not live.
    Held,
    /// The deploy was live already.
    Unchanged,
}

/// `PUT /v1/sites/{site}/deploy`.
#[derive(Debug, Serialize)]
pub struct Deployed {
    /// The deploy.
    pub deploy: DeployId,
    /// Whether it is live.
    pub published: Published,
    /// How many files the archive held.
    pub files: u64,
}

impl Deployed {
    /// What `recorded` of an archive of `files` files came to.
    pub fn of(recorded: &Recorded, files: u64) -> Self {
        let published = match recorded.published {
            control::Published::Live { .. } => Published::Live,
            control::Published::Held => Published::Held,
            _ => Published::Unchanged,
        };
        Self { deploy: recorded.deploy, published, files }
    }
}

/// `POST /v1/sites/{site}/builds`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Requested {
    /// The build is queued.
    Queued {
        /// Its id.
        build: BuildId,
    },
    /// A build with the same key is queued already.
    Duplicate,
}

/// `DELETE /v1/sites/{site}`: what was removed.
#[derive(Debug, Serialize)]
pub struct Removed {
    /// The site.
    pub site: Id<SiteKind>,
    /// Its owner.
    pub account: Id<AccountKind>,
    /// Its handle, which stays unclaimable.
    pub handle: String,
    /// When.
    pub removed: Timestamp,
    /// Deploys it had.
    pub deploys: u64,
    /// Blobs deleted.
    pub blobs: u64,
    /// Bytes they held.
    pub bytes: Bytes,
    /// Builds it had.
    pub builds: u64,
}

impl From<&Receipt> for Removed {
    fn from(receipt: &Receipt) -> Self {
        Self {
            site: receipt.site,
            account: receipt.account,
            handle: receipt.handle.to_string(),
            removed: receipt.removed,
            deploys: receipt.deploys,
            blobs: receipt.blobs,
            bytes: receipt.bytes,
            builds: receipt.builds,
        }
    }
}

/// What a rollback or a resume left: where the pointer is.
#[derive(Debug, Serialize)]
pub struct Moved {
    /// The pointer now.
    pub live: Option<LiveView>,
    /// For a resume: whether publishing had been paused.
    pub was_paused: Option<bool>,
}

/// The shape of every error answer.
#[derive(Debug, Serialize)]
pub struct Problem {
    /// A stable name for the refusal.
    pub problem: &'static str,
    /// What is wrong, in a sentence.
    pub message: String,
    /// Each cause below it, outermost first. Empty for a failure of the server's own.
    pub causes: Vec<String>,
    /// The data of the refusal, under the names of the diagnostic catalog; empty for most.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<&'static str, String>,
}

/// An enrolment taken: what the requester shows the person, and where to poll.
#[derive(Debug, Serialize)]
pub struct Enrolled {
    /// The id to poll.
    pub id: control::Id<control::EnrolmentKind>,
    /// The code the person types, as `BCDF-GHJK`.
    pub user_code: String,
    /// The fingerprint both sides show.
    pub fingerprint: String,
    /// When the ask ends if nobody approves it.
    pub expires: Timestamp,
}

/// Where an enrolment stands.
#[derive(Debug, Serialize)]
pub struct Polled {
    /// `pending` or `approved`; a denied or expired ask is an error answer.
    pub state: &'static str,
    /// When the key ends, once approved.
    pub expires: Option<Timestamp>,
}

/// Where a grant applies.
#[derive(Debug, Serialize)]
pub struct GrantView {
    /// `account` or `site`.
    pub scope: &'static str,
    /// The account or site.
    pub id: String,
    /// The rights, by name.
    pub rights: Vec<&'static str>,
}

impl From<&control::keys::Grant> for GrantView {
    fn from(grant: &control::keys::Grant) -> Self {
        let (scope, id) = match grant.scope {
            control::keys::Scope::Account(account) => ("account", account.to_string()),
            control::keys::Scope::Site(site) => ("site", site.to_string()),
            _ => ("unknown", String::new()),
        };
        Self { scope, id, rights: grant.rights.iter().map(control::keys::Right::name).collect() }
    }
}

/// Who a signature speaks for.
#[derive(Debug, Serialize)]
pub struct Whoami {
    /// The account.
    pub account: Id<AccountKind>,
    /// `operator`, `device` or `deploy`.
    pub kind: &'static str,
    /// The key's fingerprint, for a key of the database.
    pub fingerprint: Option<String>,
    /// The key's label.
    pub label: String,
    /// What it holds.
    pub grants: Vec<GrantView>,
    /// When it ends, when it does.
    pub expires: Option<Timestamp>,
}

/// A key, as a listing shows it. It has no secret and no public key.
#[derive(Debug, Serialize)]
pub struct KeyView {
    /// The fingerprint.
    pub fingerprint: String,
    /// `device` or `deploy`.
    pub kind: &'static str,
    /// The label.
    pub label: String,
    /// What it holds.
    pub grant: GrantView,
    /// When it was approved.
    pub created: Timestamp,
    /// When it ends.
    pub expires: Timestamp,
    /// The last day it was used: a date and nothing finer.
    pub last_used: Option<String>,
    /// When and why it was revoked, if it was.
    pub revoked: Option<Revoked>,
}

/// A key's end by revocation.
#[derive(Debug, Serialize)]
pub struct Revoked {
    /// When.
    pub at: Timestamp,
    /// `revoked` or `logged-out`.
    pub why: &'static str,
}

impl From<&control::keys::Key> for KeyView {
    fn from(key: &control::keys::Key) -> Self {
        Self {
            fingerprint: key.id.fingerprint().to_string(),
            kind: key.kind.name(),
            label: key.label.to_string(),
            grant: GrantView::from(&key.grant),
            created: key.created,
            expires: key.expires,
            last_used: key.last_used.map(|day| day.to_string()),
            revoked: key.revoked.map(|(at, why)| Revoked { at, why: why.name() }),
        }
    }
}
