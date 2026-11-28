//! The requests the control plane refuses.

use std::fmt;

use jiff::Timestamp;
use model::{Bytes, DeployId, Hash, Route};

use crate::hold::Why;
use crate::id::{Account, Credential, Id, Site};
use crate::keys::Right;
use crate::org::Role;
use authority_forge::Repo;

use crate::{DomainName, Handle, Reservation, Spent};

/// A request the control plane refuses, and the fact that makes it wrong. Nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// The handle is kept: for uses.ink itself, or on the brand list.
    Reserved {
        /// The handle.
        handle: Handle,
        /// Which list it is on.
        reservation: Reservation,
    },
    /// The handle belongs to a site.
    Taken {
        /// The handle.
        handle: Handle,
    },
    /// The handle was left by a site and is held (decision 0087): for a time, or for good when
    /// its owner was verified.
    Held {
        /// The handle.
        handle: Handle,
        /// Why it is held.
        why: Why,
        /// When the hold ends. `None` is for good.
        until: Option<Timestamp>,
    },
    /// The site was suspended by the operator and takes no changes (decision 0089).
    Suspended {
        /// The site.
        site: Id<Site>,
        /// Why.
        kind: crate::AbuseKind,
    },
    /// The site is not suspended.
    Serving {
        /// The site.
        site: Id<Site>,
    },
    /// The site has no git source, so it has no branches to preview.
    NoPreview {
        /// The site.
        site: Id<Site>,
    },
    /// The site has no preview with this label.
    NoLabel {
        /// The site.
        site: Id<Site>,
        /// The label.
        label: String,
    },
    /// The site's repository is private: its branches are never served at an address anyone can
    /// open (decision 0091).
    Private {
        /// The site.
        site: Id<Site>,
    },
    /// The name is one the operator serves itself, so no site may claim it as a custom domain.
    DomainOurs {
        /// The name.
        name: DomainName,
    },
    /// Another site proved the name.
    DomainTaken {
        /// The name.
        name: DomainName,
    },
    /// The site has not asked to prove the name, or its claim was dropped.
    NoClaim {
        /// The site.
        site: Id<Site>,
        /// The name.
        name: DomainName,
    },
    /// The TXT record does not carry the claim's token.
    Unproven {
        /// The name.
        name: DomainName,
        /// How many TXT records the name has.
        records: u64,
    },
    /// The site does not serve the name.
    NoDomain {
        /// The site.
        site: Id<Site>,
        /// The name.
        name: DomainName,
    },
    /// The site has as many custom domains as it may.
    DomainRoom {
        /// The site.
        site: Id<Site>,
        /// The limit.
        limit: u64,
    },
    /// Another site proved the repository.
    RepoTaken {
        /// The repository.
        repo: Repo,
    },
    /// The site has not claimed the repository, or let it go.
    NoRepo {
        /// The site.
        site: Id<Site>,
        /// The repository.
        repo: Repo,
    },
    /// The site's source is not the repository, so a push to it has nothing to build.
    RepoSource {
        /// The site.
        site: Id<Site>,
        /// The repository.
        repo: Repo,
    },
    /// The control plane holds as many open abuse reports as the operator allows.
    ReportsFull {
        /// The limit.
        limit: u64,
    },
    /// There is no organization with this account.
    NoOrg {
        /// The account.
        org: Id<Account>,
    },
    /// The account is a member of the organization already.
    Member {
        /// The organization.
        org: Id<Account>,
        /// The person.
        account: Id<Account>,
    },
    /// The account is no member of the organization.
    NotMember {
        /// The organization.
        org: Id<Account>,
        /// The person.
        account: Id<Account>,
    },
    /// The change would leave the organization with no owner.
    LastOwner {
        /// The organization.
        org: Id<Account>,
    },
    /// The member's role does not let them do this.
    Role {
        /// The organization.
        org: Id<Account>,
        /// The role they have.
        role: Role,
        /// The right it lacks.
        needs: Right,
    },
    /// A row with this id exists already.
    Exists {
        /// The id, as text.
        id: String,
    },
    /// No such account.
    NoAccount {
        /// The account.
        account: Id<Account>,
    },
    /// No such site, or it is being removed.
    NoSite {
        /// The site.
        site: Id<Site>,
    },
    /// The site is being removed and takes no change.
    Removing {
        /// The site.
        site: Id<Site>,
    },
    /// The deploy names a file whose blob the store does not hold.
    MissingBlob {
        /// The route.
        route: Route,
        /// The blob.
        blob: Hash,
    },
    /// The site has no such deploy.
    NoDeploy {
        /// The site.
        site: Id<Site>,
        /// The deploy.
        deploy: DeployId,
    },
    /// The site has no live deploy.
    NotLive {
        /// The site.
        site: Id<Site>,
    },
    /// The deploy is live already.
    AlreadyLive {
        /// The site.
        site: Id<Site>,
        /// The deploy.
        deploy: DeployId,
    },
    /// No such credential.
    NoCredential {
        /// The credential.
        credential: Id<Credential>,
    },
    /// The deploy's manifest is not in the blob store.
    NoManifest {
        /// The deploy.
        deploy: DeployId,
    },
    /// The site has no such build.
    NoBuild {
        /// The site.
        site: Id<Site>,
        /// The build.
        build: build::BuildId,
    },
    /// A build's log is not in the blob store.
    MissingLog {
        /// The build.
        build: build::BuildId,
        /// The blob.
        blob: Hash,
    },
    /// A budget is spent: the request would take the account over a limit (decision 0088).
    Over {
        /// Which budget.
        spent: Spent,
        /// What the account has or would have: bytes for the stored budget, a count for the others.
        value: u64,
        /// The limit it is over, in the same unit.
        limit: u64,
    },
    /// A source with a path that is not text cannot be stored.
    Unstorable,
    /// A site's id is not a valid tenant. Cannot happen for an id made from 16 bytes.
    Tenant(blob::TenantError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved { .. } | Self::Held { .. } => self.handle(out),
            Self::Taken { handle } => write!(out, "{handle} is taken"),
            Self::Suspended { site, kind } => write!(out, "the site {site} is suspended for {kind}"),
            Self::Serving { site } => write!(out, "the site {site} is not suspended"),
            Self::DomainOurs { .. }
            | Self::DomainTaken { .. }
            | Self::NoClaim { .. }
            | Self::Unproven { .. }
            | Self::NoDomain { .. }
            | Self::DomainRoom { .. }
            | Self::RepoTaken { .. }
            | Self::NoRepo { .. }
            | Self::RepoSource { .. }
            | Self::NoOrg { .. }
            | Self::Member { .. }
            | Self::NotMember { .. }
            | Self::LastOwner { .. }
            | Self::ReportsFull { .. }
            | Self::Role { .. } => self.domain(out).and_then(|()| self.org(out)).and_then(|()| self.abuse(out)),
            Self::Exists { id } => write!(out, "a row with the id {id} exists already"),
            Self::NoAccount { account } => write!(out, "there is no account {account}"),
            Self::NoSite { site } => write!(out, "there is no site {site}"),
            Self::Removing { site } => write!(out, "the site {site} is being removed"),
            Self::MissingBlob { .. }
            | Self::NoDeploy { .. }
            | Self::NotLive { .. }
            | Self::AlreadyLive { .. }
            | Self::NoCredential { .. }
            | Self::NoManifest { .. }
            | Self::NoBuild { .. }
            | Self::MissingLog { .. }
            | Self::NoPreview { .. }
            | Self::Private { .. }
            | Self::NoLabel { .. } => self.stored(out),
            Self::Over { spent, value, limit } => Self::over(out, *spent, (*value, *limit)),
            Self::Unstorable => out.write_str("a folder path that is not text cannot be stored"),
            Self::Tenant(_) => out.write_str("the site's id is not a valid tenant"),
        }
    }
}

impl Refusal {
    /// Words a refusal about a deploy, a build, a credential, a blob or a preview.
    fn stored(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBlob { route, blob } => {
                write!(out, "{route} is served from {blob}, which the store does not hold")
            }
            Self::NoDeploy { site, deploy } => write!(out, "the site {site} has no deploy {deploy}"),
            Self::NotLive { site } => write!(out, "the site {site} has no live deploy"),
            Self::AlreadyLive { site, deploy } => write!(out, "the deploy {deploy} is live on {site} already"),
            Self::NoCredential { credential } => write!(out, "there is no credential {credential}"),
            Self::NoManifest { deploy } => write!(out, "the manifest of deploy {deploy} is not in the blob store"),
            Self::NoBuild { site, build } => write!(out, "the site {site} has no build {build}"),
            Self::MissingLog { build, blob } => {
                write!(out, "the log of build {build} is {blob}, which the store does not hold")
            }
            Self::NoPreview { site } => {
                write!(out, "the site {site} has no git source, so it has no branches to preview")
            }
            Self::Private { site } => {
                write!(out, "the repository of site {site} is private, so its branches are not previewed")
            }
            Self::NoLabel { site, label } => write!(out, "the site {site} has no preview labelled {label}"),
            _ => Ok(()),
        }
    }
}

impl Refusal {
    /// Words a refusal about an organization.
    fn org(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOrg { org } => write!(out, "there is no organization {org}"),
            Self::Member { org, account } => write!(out, "{account} is a member of {org} already"),
            Self::NotMember { org, account } => write!(out, "{account} is not a member of {org}"),
            Self::LastOwner { org } => write!(out, "{org} would have no owner left"),
            Self::Role { org, role, needs } => {
                write!(out, "a {role} of {org} cannot do this: it needs the {needs} right over that member")
            }
            _ => Ok(()),
        }
    }
}

impl Refusal {
    /// Words a refusal about a handle.
    fn handle(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved { handle, reservation: Reservation::Infrastructure } => {
                write!(out, "{handle} is reserved for uses.ink itself")
            }
            Self::Reserved { handle, .. } => write!(out, "{handle} is a brand name that uses.ink keeps"),
            Self::Held { handle, until: Some(until), .. } => {
                write!(out, "{handle} was left by a site and is held until {until}")
            }
            Self::Held { handle, until: None, .. } => {
                write!(out, "{handle} belonged to a verified owner and is never given to anyone else")
            }
            _ => Ok(()),
        }
    }
}

impl Refusal {
    /// Words a refusal about abuse reports.
    fn abuse(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReportsFull { limit } => {
                write!(out, "the control plane holds as many open abuse reports as it may, {limit}")
            }
            _ => Ok(()),
        }
    }
}

impl Refusal {
    /// Words a refusal about a custom domain.
    fn domain(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DomainOurs { name } => write!(out, "{name} is served by uses.ink itself"),
            Self::DomainTaken { name } => write!(out, "{name} belongs to another site"),
            Self::NoClaim { site, name } => write!(out, "the site {site} has not asked to prove {name}"),
            Self::Unproven { name, records } => {
                write!(out, "the TXT record of {name} does not carry the token ({records} records found)")
            }
            Self::NoDomain { site, name } => write!(out, "the site {site} does not serve {name}"),
            Self::DomainRoom { site, limit } => {
                write!(out, "the site {site} has as many custom domains as it may, {limit}")
            }
            Self::RepoTaken { repo } => write!(out, "{repo} belongs to another site"),
            Self::NoRepo { site, repo } => write!(out, "the site {site} has not claimed {repo}"),
            Self::RepoSource { site, repo } => write!(out, "the source of the site {site} is not {repo}"),
            _ => Ok(()),
        }
    }
}

impl Refusal {
    /// Words a refusal for going over a budget.
    fn over(out: &mut fmt::Formatter<'_>, spent: Spent, reached: (u64, u64)) -> fmt::Result {
        let (value, limit) = reached;
        match spent {
            Spent::Stored => {
                write!(out, "the account would store {} over its limit of {}", Bytes(value), Bytes(limit))
            }
            Spent::Builds => write!(out, "the account started {value} builds in a day, which is its limit of {limit}"),
            Spent::Sites => write!(out, "the account has {value} sites, which is its limit of {limit}"),
        }
    }

    /// The code of a refusal about a claimed repository.
    const fn repo_code(&self) -> &'static str {
        match self {
            Self::RepoTaken { .. } => "repo.claim.taken",
            Self::NoRepo { .. } => "repo.claim.unknown",
            _ => "repo.site.mismatch",
        }
    }

    /// The code of a refusal about an organization.
    const fn org_code(&self) -> &'static str {
        match self {
            Self::NoOrg { .. } => "org.id.unknown",
            Self::Member { .. } => "org.member.exists",
            Self::NotMember { .. } => "org.member.unknown",
            Self::LastOwner { .. } => "org.owner.last",
            _ => "org.role.denied",
        }
    }

    /// The code of the catalog (`crates/core/diag/catalog.toml`) that words this refusal.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Reserved { reservation: Reservation::Infrastructure, .. } => "handle.name.reserved",
            Self::Reserved { .. } => "handle.name.brand",
            Self::Taken { .. } => "handle.name.taken",
            Self::Held { until: None, .. } => "handle.name.kept",
            Self::Held { .. } => "handle.name.held",
            Self::Suspended { .. } => "site.state.suspended",
            Self::Serving { .. } => "site.state.serving",
            Self::NoPreview { .. } => "preview.site.unsupported",
            Self::Private { .. } => "preview.site.private",
            Self::NoLabel { .. } => "preview.label.unknown",
            Self::DomainOurs { .. } => "domain.name.ours",
            Self::DomainTaken { .. } => "domain.name.taken",
            Self::NoClaim { .. } => "domain.claim.unknown",
            Self::Unproven { .. } => "domain.proof.missing",
            Self::NoDomain { .. } => "domain.name.unknown",
            Self::DomainRoom { .. } => "domain.site.full",
            Self::ReportsFull { .. } => "abuse.reports.full",
            Self::RepoTaken { .. } | Self::NoRepo { .. } | Self::RepoSource { .. } => self.repo_code(),
            Self::NoOrg { .. }
            | Self::Member { .. }
            | Self::NotMember { .. }
            | Self::LastOwner { .. }
            | Self::Role { .. } => self.org_code(),
            Self::Exists { .. } => "row.id.exists",
            Self::NoAccount { .. } => "account.id.unknown",
            Self::NoSite { .. } => "site.id.unknown",
            Self::Removing { .. } => "site.state.removing",
            Self::MissingBlob { .. } => "deploy.blob.missing",
            Self::NoDeploy { .. } => "deploy.id.unknown",
            Self::NotLive { .. } => "site.deploy.absent",
            Self::AlreadyLive { .. } => "deploy.state.live",
            Self::NoCredential { .. } => "credential.id.unknown",
            Self::NoManifest { .. } => "deploy.manifest.missing",
            Self::NoBuild { .. } => "build.id.unknown",
            Self::MissingLog { .. } => "build.log.missing",
            Self::Over { spent, .. } => spent.code(),
            Self::Unstorable => "source.path.unstorable",
            Self::Tenant(_) => "site.tenant.invalid",
        }
    }

    /// The data of the refusal, named as the catalog names the fields of its code.
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Reserved { handle, .. } | Self::Taken { handle } => vec![("handle", handle.to_string())],
            Self::Held { handle, until, .. } => {
                let mut fields = vec![("handle", handle.to_string())];
                fields.extend(until.map(|until| ("until", until.to_string())));
                fields
            }
            Self::Suspended { site, kind } => vec![("id", site.to_string()), ("reason", kind.to_string())],
            Self::NoLabel { site, label } => vec![("id", site.to_string()), ("label", label.clone())],
            Self::Exists { id } => vec![("id", id.clone())],
            Self::NoAccount { account } => vec![("id", account.to_string())],
            Self::NoSite { site }
            | Self::Removing { site }
            | Self::NotLive { site }
            | Self::Serving { site }
            | Self::NoPreview { site }
            | Self::Private { site } => {
                vec![("id", site.to_string())]
            }
            Self::Over { spent, value, limit } => {
                let name = if *spent == Spent::Stored { "size" } else { "count" };
                vec![(name, value.to_string()), ("limit", limit.to_string())]
            }
            Self::Unstorable | Self::Tenant(_) => Vec::new(),
            _ => self.named(),
        }
    }
}

impl Refusal {
    /// The data of a refusal about a custom domain or an organization.
    fn named(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::DomainOurs { name } | Self::DomainTaken { name } => vec![("name", name.to_string())],
            Self::NoClaim { site, name } | Self::NoDomain { site, name } => {
                vec![("id", site.to_string()), ("name", name.to_string())]
            }
            Self::Unproven { name, records } => vec![("name", name.to_string()), ("count", records.to_string())],
            Self::DomainRoom { site, limit } => vec![("id", site.to_string()), ("limit", limit.to_string())],
            Self::RepoTaken { repo } => vec![("repo", repo.to_string())],
            Self::NoRepo { site, repo } | Self::RepoSource { site, repo } => {
                vec![("id", site.to_string()), ("repo", repo.to_string())]
            }
            Self::ReportsFull { limit } => vec![("limit", limit.to_string())],
            Self::NoOrg { org } | Self::LastOwner { org } => vec![("id", org.to_string())],
            Self::Member { org, account } | Self::NotMember { org, account } => {
                vec![("id", org.to_string()), ("account", account.to_string())]
            }
            Self::Role { org, role, needs } => {
                vec![("id", org.to_string()), ("role", role.to_string()), ("right", needs.to_string())]
            }
            Self::NoDeploy { site, deploy } | Self::AlreadyLive { site, deploy } => {
                vec![("id", site.to_string()), ("deploy", deploy.to_string())]
            }
            Self::MissingBlob { route, blob } => vec![("route", route.to_string()), ("hash", blob.to_string())],
            Self::NoCredential { credential } => vec![("id", credential.to_string())],
            Self::NoManifest { deploy } => vec![("deploy", deploy.to_string())],
            Self::NoBuild { site, build } => vec![("id", site.to_string()), ("build", build.to_string())],
            Self::MissingLog { build, blob } => vec![("build", build.to_string()), ("hash", blob.to_string())],
            _ => Vec::new(),
        }
    }
}

impl std::error::Error for Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Tenant(source) => Some(source),
            _ => None,
        }
    }
}
