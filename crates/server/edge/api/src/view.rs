//! What the API answers with, as typed values that serialize and that a client reads back: one
//! contract for both sides.

use std::collections::BTreeMap;

use build::{BuildId, Source, Trigger};
use control::{AccountKind, Deploy, Id, Job, JobState, Live, Receipt, Recorded, Site, SiteKind, SiteState};
use jiff::Timestamp;
use model::{Bytes, DeployId, ExternalUrl};
use serde::{Deserialize, Serialize};

/// Where a site's content comes from.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
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
            Source::Upload | Source::Bundle(_) => Self::Upload,
            _ => Self::Folder,
        }
    }
}

/// One site.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// The branch it builds as a preview, when it is not a build of the site itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
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
            branch: job.pin.as_ref().map(|pin| pin.branch.to_string()),
        }
    }
}

/// `GET /v1/sites/{site}`: what a person looks at first.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    /// The site.
    pub site: SiteView,
    /// What it serves, if anything.
    pub live: Option<LiveView>,
    /// The latest build, if there is one.
    pub build: Option<BuildView>,
    /// Builds that are queued or running, newest first, among the recent ones.
    pub pending: Vec<BuildView>,
}

/// A list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Listed<T> {
    /// The rows, newest first (sites: oldest first).
    pub items: Vec<T>,
}

/// What recording a deploy did.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// For a key of an organization: whether the member who approved it has left. The key stays
    /// until an owner revokes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_removed: Option<bool>,
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
            creator_removed: None,
        }
    }
}

/// `POST /v1/abuse`: a report taken.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reported {
    /// The report, for the reporter to quote.
    pub id: Id<control::ReportKind>,
}

/// A report, as the operator reads it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportView {
    /// The report.
    pub id: Id<control::ReportKind>,
    /// The handle it names.
    pub handle: String,
    /// What it is about.
    pub kind: control::AbuseKind,
    /// What happened.
    pub detail: String,
    /// How to reach the reporter.
    pub contact: Option<String>,
    /// `open` or `actioned`.
    pub state: &'static str,
    /// When it came.
    pub at: Timestamp,
}

impl From<&control::Report> for ReportView {
    fn from(report: &control::Report) -> Self {
        Self {
            id: report.id,
            handle: report.handle.to_string(),
            kind: report.kind,
            detail: report.detail.as_str().to_owned(),
            contact: report.contact.as_ref().map(|contact| contact.as_str().to_owned()),
            state: report.state.name(),
            at: report.at,
        }
    }
}

/// A suspension made or lifted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Suspended {
    /// The site.
    pub site: Id<SiteKind>,
    /// Why it was suspended; absent once it is lifted.
    pub kind: Option<control::AbuseKind>,
    /// When.
    pub at: Timestamp,
}

/// A preview, as its owner reads it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Previewed {
    /// The label.
    pub slug: String,
    /// Where it is served, anyone with the address can open it.
    pub url: Option<String>,
    /// The deploy it serves.
    pub deploy: DeployId,
    /// How many files.
    pub files: u64,
    /// When it ends.
    pub expires: Timestamp,
}

impl Previewed {
    /// `preview`, served at `url`.
    pub fn of(preview: &control::Preview, url: Option<ExternalUrl>) -> Self {
        Self {
            slug: preview.slug.to_string(),
            url: url.map(|url| url.as_str().to_owned()),
            deploy: preview.deploy,
            files: preview.files,
            expires: preview.expires,
        }
    }
}

/// A preview that was removed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unpreviewed {
    /// The label.
    pub slug: String,
    /// How many blobs only it held and were deleted.
    pub blobs: u64,
}

/// The record to publish to prove a name: a TXT record at a host, with a value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordView {
    /// Where to publish it.
    pub host: String,
    /// What it says.
    pub value: String,
}

/// A custom domain of a site, as its owner reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainView {
    /// The name.
    pub name: String,
    /// `proven` when the name serves the site, `pending` when it has still to be proved.
    pub state: String,
    /// When the proof was seen, for a proven name.
    pub proven: Option<Timestamp>,
    /// The record that proves a pending name.
    pub record: Option<RecordView>,
}

impl DomainView {
    /// A name that serves the site.
    pub fn proven(domain: &control::Proven) -> Self {
        Self { name: domain.name.to_string(), state: "proven".to_owned(), proven: Some(domain.at), record: None }
    }

    /// A name the site asked to prove, with the record that proves it.
    pub fn pending(claim: &control::Claim) -> Self {
        let challenge = authority_dns::Challenge::of(claim);
        Self {
            name: claim.name.to_string(),
            state: "pending".to_owned(),
            proven: None,
            record: Some(RecordView { host: challenge.host.to_string(), value: challenge.value }),
        }
    }
}

/// A repository a site claimed (decision 0160).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoView {
    /// `host/owner/name`.
    pub repo: String,
    /// The forge it lives on (`github`, `forgejo` or `gitlab`), when this server reaches its host.
    pub forge: Option<String>,
    /// `proven` when a push to it builds the site, `pending` when it has still to be proved.
    pub state: String,
    /// The line to commit, on its own line, to the file below on the default branch.
    pub challenge: String,
    /// The file the challenge is committed to.
    pub file: String,
    /// When it was asked.
    pub created: Timestamp,
    /// When the forge showed the challenge.
    pub proven: Option<Timestamp>,
    /// The path of the webhook the forge delivers pushes to, on the API host.
    pub hook: Option<String>,
}

impl RepoView {
    /// A claim as listed.
    pub fn of(claim: &control::RepoClaim, forge: Option<authority_repo::Forge>) -> Self {
        Self {
            repo: claim.repo.to_string(),
            forge: forge.map(|forge| forge.to_string()),
            state: if claim.bound() { "proven" } else { "pending" }.to_owned(),
            challenge: claim.challenge.to_string(),
            file: authority_forge::Challenge::FILE.to_owned(),
            created: claim.created,
            proven: claim.proven,
            hook: forge.map(|forge| format!("/v1/hooks/{forge}")),
        }
    }
}

/// A secret shown once. It prints as redacted.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Shown(String);

impl Shown {
    /// The secret, for the one place that is told it.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Shown {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Shown(<redacted>)")
    }
}

/// A claim just made or a secret just rotated: the claim and the webhook secret to give the
/// forge, which no other call tells again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoGiven {
    /// The claim.
    #[serde(flatten)]
    pub claim: RepoView,
    /// The secret, in hex, for the forge's webhook form.
    pub secret: Shown,
}

impl RepoGiven {
    /// A claim and its secret.
    pub fn of(claim: RepoView, secret: &authority_forge::Secret) -> Self {
        Self { claim, secret: Shown(secret.text().unwrap_or_default().to_owned()) }
    }
}

/// A repository that was let go.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoReleased {
    /// `host/owner/name`.
    pub repo: String,
}

/// A custom domain that was removed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unclaimed {
    /// The name.
    pub name: String,
}

/// An organization, with the caller's place in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgView {
    /// Its account id.
    pub id: Id<AccountKind>,
    /// What it is called.
    pub name: String,
    /// The caller's role there.
    pub role: String,
    /// How many members it has.
    pub members: u64,
    /// When it was made.
    pub created: Timestamp,
}

impl From<&control::Standing> for OrgView {
    fn from(standing: &control::Standing) -> Self {
        Self {
            id: standing.org.id,
            name: standing.org.name.clone(),
            role: standing.role.name().to_owned(),
            members: standing.members,
            created: standing.org.created,
        }
    }
}

/// A member of an organization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberView {
    /// The person's account id.
    pub account: Id<AccountKind>,
    /// What they may do there.
    pub role: String,
    /// When they joined.
    pub since: Timestamp,
}

impl From<&control::Member> for MemberView {
    fn from(member: &control::Member) -> Self {
        Self { account: member.account, role: member.role.name().to_owned(), since: member.since }
    }
}

/// `GET /v1/orgs/{org}`: the organization and everyone in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgMembers {
    /// The organization, as the caller sees it.
    pub org: OrgView,
    /// Its members, owners first.
    pub members: Vec<MemberView>,
}

/// A member that was removed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unmembered {
    /// The organization.
    pub org: Id<AccountKind>,
    /// The person who is no longer in it.
    pub account: Id<AccountKind>,
}
