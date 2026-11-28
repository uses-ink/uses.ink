//! The API's routes: a method and a path become one [`Call`], or say why they are none.

use std::fmt;

use build::{BuildId, BuildIdError};
use control::keys::Right;
use control::{AccountKind, DomainName, EnrolmentKind, Id, IdError, NameError, SiteKind, Slug, SlugError};
use model::DeployId;
use sig::Fingerprint;

use crate::reply::Method;

/// What a request asks for. Every id is parsed; nothing here is text the control plane would
/// have to trust.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Call {
    /// `GET /v1/sites`: the caller's sites.
    Sites,
    /// `POST /v1/sites`: add a site.
    Add,
    /// `GET /v1/sites/{site}`: the site, its live deploy and its latest build.
    Status(Id<SiteKind>),
    /// `DELETE /v1/sites/{site}`: remove the site and everything of it.
    Remove(Id<SiteKind>),
    /// `GET /v1/sites/{site}/deploys`: recent deploys, newest first.
    Deploys(Id<SiteKind>),
    /// `POST /v1/sites/{site}/rollback`: make an older deploy live and pause publishing.
    Rollback(Id<SiteKind>),
    /// `POST /v1/sites/{site}/resume`: let builds publish again.
    Resume(Id<SiteKind>),
    /// `GET /v1/sites/{site}/builds`: recent builds, newest first.
    Builds(Id<SiteKind>),
    /// `POST /v1/sites/{site}/builds`: request a build.
    Build(Id<SiteKind>),
    /// `GET /v1/sites/{site}/builds/{build}/log`: the build's log as NDJSON.
    Log {
        /// The site.
        site: Id<SiteKind>,
        /// The build.
        build: BuildId,
    },
    /// `GET /v1/sites/{site}/export` and `GET /v1/sites/{site}/deploys/{deploy}/export`: the
    /// files of a deploy as a tar archive.
    Export {
        /// The site.
        site: Id<SiteKind>,
        /// A deploy that is not the live one.
        deploy: Option<DeployId>,
    },
    /// `GET /v1/sites/{site}/domains`: the custom domains the site serves and has still to prove.
    Domains(Id<SiteKind>),
    /// `PUT /v1/sites/{site}/domains/{name}`: ask to prove a name; answered with the record to
    /// publish.
    Claim {
        /// The site.
        site: Id<SiteKind>,
        /// The name.
        name: DomainName,
    },
    /// `POST /v1/sites/{site}/domains/{name}/proof`: read the record and make the name serve.
    Prove {
        /// The site.
        site: Id<SiteKind>,
        /// The name.
        name: DomainName,
    },
    /// `DELETE /v1/sites/{site}/domains/{name}`: stop serving the name, or drop the claim.
    Unclaim {
        /// The site.
        site: Id<SiteKind>,
        /// The name.
        name: DomainName,
    },
    /// `GET /v1/sites/{site}/previews`: the previews that have not ended.
    Previews(Id<SiteKind>),
    /// `PUT /v1/sites/{site}/previews/{label}`: a built folder as a tar archive, made the preview
    /// of a branch.
    Preview {
        /// The site.
        site: Id<SiteKind>,
        /// The label.
        slug: Slug,
    },
    /// `DELETE /v1/sites/{site}/previews/{label}`: the preview ends now.
    Unpreview {
        /// The site.
        site: Id<SiteKind>,
        /// The label.
        slug: Slug,
    },
    /// `POST /v1/sites/{site}/source`: a tree of files, answered with the blobs it is missing or
    /// a build of it.
    Source(Id<SiteKind>),
    /// `PUT /v1/sites/{site}/source`: a tar archive of blobs named by their hashes.
    Parts(Id<SiteKind>),
    /// `GET /v1/sites/{site}/source`: the files of the latest tree, as a tar archive.
    Bundle(Id<SiteKind>),
    /// `PUT /v1/sites/{site}/deploy`: a built folder as a tar archive, made a deploy.
    Intake(Id<SiteKind>),
    /// `POST /v1/abuse`: anyone reports a site. The other call that is not signed.
    Report,
    /// `GET /v1/abuse`: the operator reads the reports.
    Reports,
    /// `POST /v1/sites/{site}/suspend`: the operator takes a site down.
    Suspend(Id<SiteKind>),
    /// `POST /v1/sites/{site}/reinstate`: the operator lifts a suspension.
    Reinstate(Id<SiteKind>),
    /// `GET /v1/orgs`: the organizations the caller is a member of.
    Orgs,
    /// `POST /v1/orgs`: make an organization with the caller as its owner.
    OrgAdd,
    /// `GET /v1/orgs/{org}`: the organization and its members.
    OrgShow(Id<AccountKind>),
    /// `POST /v1/orgs/{org}/members`: add a member.
    MemberAdd(Id<AccountKind>),
    /// `PUT /v1/orgs/{org}/members/{account}`: change a member's role.
    MemberRole {
        /// The organization.
        org: Id<AccountKind>,
        /// The member.
        account: Id<AccountKind>,
    },
    /// `DELETE /v1/orgs/{org}/members/{account}`: remove a member.
    MemberOut {
        /// The organization.
        org: Id<AccountKind>,
        /// The member.
        account: Id<AccountKind>,
    },
    /// `POST /v1/enrol`: a key asks to be enrolled. The one call that is not signed.
    Enrol,
    /// `GET /v1/enrol/{id}`: the outcome of an ask, signed by the key that asked.
    Polled(Id<EnrolmentKind>),
    /// `GET /v1/whoami`: who the signature speaks for and what it may do.
    Whoami,
    /// `GET /v1/keys`: the account's keys.
    Keys,
    /// `DELETE /v1/keys/{fingerprint}`: revoke a key.
    Revoke(Fingerprint),
}

/// A path that is not a route.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RouteError {
    /// Nothing is at this path.
    NoRoute {
        /// The path.
        path: String,
    },
    /// The path is a route, for other methods.
    Method {
        /// The method asked.
        asked: Method,
        /// The methods the path takes.
        allowed: Vec<Method>,
    },
    /// A part of the path should be an id and is not.
    Id {
        /// What it should name.
        noun: &'static str,
        /// Why not.
        source: IdError,
    },
    /// A part of the path should be a key fingerprint and is not.
    Fingerprint {
        /// The text.
        written: String,
        /// Why not.
        source: sig::FingerprintError,
    },
    /// A part of the path should be a build and is not.
    Build {
        /// The text.
        written: String,
        /// Why not.
        source: BuildIdError,
    },
    /// A part of the path should be a preview label and is not.
    Label {
        /// The text.
        written: String,
        /// Why not.
        source: SlugError,
    },
    /// A part of the path should be a domain name and is not.
    Domain {
        /// The text.
        written: String,
        /// Why not.
        source: NameError,
    },
    /// A part of the path should be a deploy and is not.
    Deploy {
        /// The text.
        written: String,
        /// Why not.
        source: model::HashError,
    },
}

impl fmt::Display for RouteError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRoute { path } => write!(out, "there is no route {path}"),
            Self::Method { asked, allowed } => {
                let allowed: Vec<String> = allowed.iter().map(ToString::to_string).collect();
                write!(out, "{asked} is not taken here, only {}", allowed.join(", "))
            }
            Self::Id { noun, .. } => write!(out, "the {noun} in the path is not an id"),
            Self::Fingerprint { written, .. } => write!(out, "{written:?} in the path is not a key fingerprint"),
            Self::Build { written, .. } => write!(out, "{written:?} in the path is not a build id"),
            Self::Deploy { written, .. } => write!(out, "{written:?} in the path is not a deploy id"),
            Self::Label { written, .. } => write!(out, "{written:?} in the path is not a preview label"),
            Self::Domain { written, .. } => write!(out, "{written:?} in the path is not a domain name"),
        }
    }
}

impl std::error::Error for RouteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Id { source, .. } => Some(source),
            Self::Fingerprint { source, .. } => Some(source),
            Self::Build { source, .. } => Some(source),
            Self::Label { source, .. } => Some(source),
            Self::Deploy { source, .. } => Some(source),
            Self::Domain { source, .. } => Some(source),
            Self::NoRoute { .. } | Self::Method { .. } => None,
        }
    }
}

/// Picks the call a method makes among the ones a path takes.
type Pick<'a> = dyn Fn(&[(Method, Call)]) -> Result<Call, RouteError> + 'a;

impl Call {
    /// The call `method` of `path` is.
    pub fn parse(method: Method, path: &str) -> Result<Self, RouteError> {
        let parts: Vec<&str> = path.trim_end_matches('/').split('/').skip(1).collect();
        let no_route = || RouteError::NoRoute { path: path.chars().take(200).collect() };
        let by = |allowed: &[(Method, Self)]| {
            allowed.iter().find(|(known, _)| *known == method).map(|(_, call)| call.clone()).ok_or_else(|| {
                RouteError::Method { asked: method, allowed: allowed.iter().map(|(known, _)| *known).collect() }
            })
        };
        let rest = parts.get(2..).unwrap_or_default();
        match (parts.first(), parts.get(1)) {
            (Some(&"v1"), Some(&"sites")) => {}
            (Some(&"v1"), Some(&"whoami")) if rest.is_empty() => return by(&[(Method::Get, Self::Whoami)]),
            (Some(&"v1"), Some(&"abuse")) if rest.is_empty() => {
                return by(&[(Method::Post, Self::Report), (Method::Get, Self::Reports)]);
            }
            (Some(&"v1"), Some(&"orgs")) => return Self::orgs(method, rest, &by),
            (Some(&"v1"), Some(&noun @ ("enrol" | "keys"))) => {
                return Self::credentials(noun, rest, &by).ok_or_else(no_route)?;
            }
            _ => return Err(no_route()),
        }
        match rest {
            [] => by(&[(Method::Get, Self::Sites), (Method::Post, Self::Add)]),
            [site] => {
                let site = Self::site(site)?;
                by(&[(Method::Get, Self::Status(site)), (Method::Delete, Self::Remove(site))])
            }
            [site, tail @ ..] => Self::nested(Self::site(site)?, tail, &by).ok_or_else(no_route)?,
        }
    }

    /// The routes under `/v1/orgs`: the organizations, their members, and any call of the
    /// account routes made as the organization (`/v1/orgs/{org}/sites`, `keys`, `whoami`).
    fn orgs(method: Method, rest: &[&str], by: &Pick<'_>) -> Result<Self, RouteError> {
        let org = |text: &str| text.parse().map_err(|source| RouteError::Id { noun: "organization", source });
        let member = |text: &str| text.parse().map_err(|source| RouteError::Id { noun: "member", source });
        match rest {
            [] => by(&[(Method::Get, Self::Orgs), (Method::Post, Self::OrgAdd)]),
            [id] => by(&[(Method::Get, Self::OrgShow(org(id)?))]),
            [id, "members"] => by(&[(Method::Post, Self::MemberAdd(org(id)?))]),
            [id, "members", account] => {
                let (org, account) = (org(id)?, member(account)?);
                by(&[
                    (Method::Put, Self::MemberRole { org, account }),
                    (Method::Delete, Self::MemberOut { org, account }),
                ])
            }
            [id, noun @ ("sites" | "keys" | "whoami"), tail @ ..] => {
                org(id)?;
                let inner = format!("/v1/{noun}/{}", tail.join("/"));
                Self::parse(method, &inner)
            }
            _ => Err(RouteError::NoRoute { path: format!("/v1/orgs/{}", rest.join("/")).chars().take(200).collect() }),
        }
    }

    /// The organization a path acts as, when it is one of the account routes under
    /// `/v1/orgs/{org}`: the sites, keys and `whoami` of the organization.
    pub fn acting(path: &str) -> Option<Id<AccountKind>> {
        let parts: Vec<&str> = path.trim_end_matches('/').split('/').skip(1).collect();
        match parts.as_slice() {
            ["v1", "orgs", org, "sites" | "keys" | "whoami", ..] => org.parse().ok(),
            _ => None,
        }
    }

    /// The organization a call is about by its own path: the one whose members it reads or changes.
    pub const fn org(&self) -> Option<Id<AccountKind>> {
        match self {
            Self::OrgShow(org) | Self::MemberAdd(org) | Self::MemberRole { org, .. } | Self::MemberOut { org, .. } => {
                Some(*org)
            }
            _ => None,
        }
    }

    /// The routes of enrolling and of keys.
    fn credentials(noun: &str, rest: &[&str], by: &Pick<'_>) -> Option<Result<Self, RouteError>> {
        Some(match (noun, rest) {
            ("enrol", []) => by(&[(Method::Post, Self::Enrol)]),
            ("enrol", [id]) => match id.parse() {
                Ok(id) => by(&[(Method::Get, Self::Polled(id))]),
                Err(source) => Err(RouteError::Id { noun: "enrolment", source }),
            },
            ("keys", []) => by(&[(Method::Get, Self::Keys)]),
            ("keys", [print]) => match print.parse() {
                Ok(print) => by(&[(Method::Delete, Self::Revoke(print))]),
                Err(source) => Err(RouteError::Fingerprint { written: (*print).to_owned(), source }),
            },
            _ => return None,
        })
    }

    /// The routes under a site's `domains`.
    fn domain(site: Id<SiteKind>, tail: &[&str], by: &Pick<'_>) -> Option<Result<Self, RouteError>> {
        let named = |name: &str| {
            name.parse::<DomainName>().map_err(|source| RouteError::Domain { written: name.to_owned(), source })
        };
        let routed = |name: &str| -> Result<Self, RouteError> {
            let name = named(name)?;
            by(&[
                (Method::Put, Self::Claim { site, name: name.clone() }),
                (Method::Delete, Self::Unclaim { site, name }),
            ])
        };
        Some(match tail {
            [] => by(&[(Method::Get, Self::Domains(site))]),
            [name] => routed(name),
            [name, "proof"] => named(name).and_then(|name| by(&[(Method::Post, Self::Prove { site, name })])),
            _ => return None,
        })
    }

    /// The routes under one site: its noun, and what follows.
    fn nested(site: Id<SiteKind>, tail: &[&str], by: &Pick<'_>) -> Option<Result<Self, RouteError>> {
        Some(match tail {
            ["deploys"] => by(&[(Method::Get, Self::Deploys(site))]),
            ["deploys", deploy, "export"] => match Self::deploy(deploy) {
                Ok(deploy) => by(&[(Method::Get, Self::Export { site, deploy: Some(deploy) })]),
                Err(error) => Err(error),
            },
            ["rollback"] => by(&[(Method::Post, Self::Rollback(site))]),
            ["resume"] => by(&[(Method::Post, Self::Resume(site))]),
            ["suspend"] => by(&[(Method::Post, Self::Suspend(site))]),
            ["reinstate"] => by(&[(Method::Post, Self::Reinstate(site))]),
            ["builds"] => by(&[(Method::Get, Self::Builds(site)), (Method::Post, Self::Build(site))]),
            ["builds", build, "log"] => match build.parse::<BuildId>() {
                Ok(build) => by(&[(Method::Get, Self::Log { site, build })]),
                Err(source) => Err(RouteError::Build { written: (*build).to_owned(), source }),
            },
            ["domains", rest @ ..] => return Self::domain(site, rest, by),
            ["previews"] => by(&[(Method::Get, Self::Previews(site))]),
            ["previews", slug] => match slug.parse::<Slug>() {
                Ok(slug) => by(&[
                    (Method::Put, Self::Preview { site, slug: slug.clone() }),
                    (Method::Delete, Self::Unpreview { site, slug }),
                ]),
                Err(source) => Err(RouteError::Label { written: (*slug).to_owned(), source }),
            },
            ["export"] => by(&[(Method::Get, Self::Export { site, deploy: None })]),
            ["deploy"] => by(&[(Method::Put, Self::Intake(site))]),
            ["source"] => by(&[
                (Method::Post, Self::Source(site)),
                (Method::Put, Self::Parts(site)),
                (Method::Get, Self::Bundle(site)),
            ]),
            _ => return None,
        })
    }

    /// The right a call needs and the site it is about, when it is about one. `None` for the
    /// calls that need none: asking to enrol, polling one's own ask, and knowing who one is.
    pub const fn needs(&self) -> Option<(Right, Option<Id<SiteKind>>)> {
        Some(match self {
            Self::Sites | Self::Orgs | Self::OrgShow(_) => (Right::Read, None),
            Self::Add | Self::OrgAdd => (Right::Edit, None),
            Self::Status(site)
            | Self::Deploys(site)
            | Self::Builds(site)
            | Self::Log { site, .. }
            | Self::Bundle(site)
            | Self::Previews(site)
            | Self::Domains(site)
            | Self::Export { site, .. } => (Right::Read, Some(*site)),
            Self::Claim { site, .. } | Self::Prove { site, .. } | Self::Unclaim { site, .. } => {
                (Right::Edit, Some(*site))
            }
            Self::Remove(site) => (Right::Destroy, Some(*site)),
            Self::Rollback(site)
            | Self::Resume(site)
            | Self::Build(site)
            | Self::Intake(site)
            | Self::Preview { site, .. }
            | Self::Unpreview { site, .. }
            | Self::Source(site)
            | Self::Parts(site) => (Right::Deploy, Some(*site)),
            Self::Keys | Self::Revoke(_) => (Right::Keys, None),
            Self::MemberAdd(_) | Self::MemberRole { .. } | Self::MemberOut { .. } => (Right::Members, None),
            Self::Enrol
            | Self::Polled(_)
            | Self::Whoami
            | Self::Report
            | Self::Reports
            | Self::Suspend(_)
            | Self::Reinstate(_) => return None,
        })
    }

    /// Whether only the operator's keys may make the call: it is about every account's sites.
    pub const fn operator(&self) -> bool {
        matches!(self, Self::Reports | Self::Suspend(_) | Self::Reinstate(_))
    }

    fn site(text: &str) -> Result<Id<SiteKind>, RouteError> {
        text.parse().map_err(|source| RouteError::Id { noun: "site", source })
    }

    fn deploy(text: &str) -> Result<DeployId, RouteError> {
        text.parse().map_err(|source| RouteError::Deploy { written: text.to_owned(), source })
    }
}

impl logging::Kind for &Call {
    /// The route's name, with no site or build in it.
    fn kind(self) -> &'static str {
        match self {
            Call::Sites => "sites",
            Call::Add => "add",
            Call::Status(_) => "status",
            Call::Remove(_) => "remove",
            Call::Deploys(_) => "deploys",
            Call::Rollback(_) => "rollback",
            Call::Resume(_) => "resume",
            Call::Builds(_) => "builds",
            Call::Build(_) => "build",
            Call::Log { .. } => "log",
            Call::Export { .. } => "export",
            Call::Intake(_) => "intake",
            Call::Domains(_) => "domains",
            Call::Claim { .. } => "claim",
            Call::Prove { .. } => "prove",
            Call::Unclaim { .. } => "unclaim",
            Call::Previews(_) => "previews",
            Call::Preview { .. } => "preview",
            Call::Unpreview { .. } => "unpreview",
            Call::Source(_) => "source",
            Call::Parts(_) => "parts",
            Call::Bundle(_) => "bundle",
            Call::Report => "report",
            Call::Reports => "reports",
            Call::Suspend(_) => "suspend",
            Call::Reinstate(_) => "reinstate",
            Call::Enrol => "enrol",
            Call::Polled(_) => "polled",
            Call::Whoami => "whoami",
            Call::Keys => "keys",
            Call::Revoke(_) => "revoke",
            Call::Orgs => "orgs",
            Call::OrgAdd => "org-add",
            Call::OrgShow(_) => "org",
            Call::MemberAdd(_) => "member-add",
            Call::MemberRole { .. } => "member-role",
            Call::MemberOut { .. } => "member-out",
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const SITE: &str = "01010101010101010101010101010101";

    #[test]
    fn routes_are_methods_and_paths() {
        let site = Id::from_bytes([1; 16]);
        let call = |method, path: &str| Call::parse(method, path);
        assert_eq!(call(Method::Get, "/v1/sites"), Ok(Call::Sites));
        assert_eq!(call(Method::Post, "/v1/sites/"), Ok(Call::Add));
        assert_eq!(call(Method::Delete, &format!("/v1/sites/{SITE}")), Ok(Call::Remove(site)));
        assert_eq!(call(Method::Put, &format!("/v1/sites/{SITE}/deploy")), Ok(Call::Intake(site)));
        assert_eq!(call(Method::Get, &format!("/v1/sites/{SITE}/export")), Ok(Call::Export { site, deploy: None }));
    }

    #[test]
    fn organizations_have_routes_and_act_for_the_account_routes() {
        let org: Id<AccountKind> = Id::from_bytes([9; 16]);
        let (id, person) = ("09".repeat(16), "03".repeat(16));
        let at = |method, path: &str| Call::parse(method, path);
        assert_eq!(at(Method::Post, "/v1/orgs"), Ok(Call::OrgAdd));
        assert_eq!(at(Method::Get, &format!("/v1/orgs/{id}")), Ok(Call::OrgShow(org)));
        assert_eq!(at(Method::Post, &format!("/v1/orgs/{id}/members")), Ok(Call::MemberAdd(org)));
        let account = Id::from_bytes([3; 16]);
        assert_eq!(
            at(Method::Delete, &format!("/v1/orgs/{id}/members/{person}")),
            Ok(Call::MemberOut { org, account })
        );
        assert_eq!(at(Method::Get, &format!("/v1/orgs/{id}/sites/{SITE}")), Ok(Call::Status(Id::from_bytes([1; 16]))));
        assert_eq!(Call::acting(&format!("/v1/orgs/{id}/sites")), Some(org));
        assert_eq!(Call::acting("/v1/sites"), None);
        assert_eq!(Call::acting(&format!("/v1/orgs/{id}/members")), None, "members are managed by their own routes");
        assert!(matches!(at(Method::Get, "/v1/orgs/zz/sites"), Err(RouteError::Id { noun: "organization", .. })));
        assert_eq!(Call::MemberAdd(org).needs(), Some((Right::Members, None)));
        assert_eq!(Call::MemberAdd(org).org(), Some(org));
    }

    #[test]
    fn a_wrong_method_lists_the_right_ones() {
        let error = Call::parse(Method::Delete, "/v1/sites").unwrap_err();
        assert_eq!(error, RouteError::Method { asked: Method::Delete, allowed: vec![Method::Get, Method::Post] });
    }

    #[test]
    fn what_names_nothing_is_no_route_or_no_id() {
        assert!(matches!(Call::parse(Method::Get, "/v2/sites"), Err(RouteError::NoRoute { .. })));
        assert!(matches!(Call::parse(Method::Get, "/"), Err(RouteError::NoRoute { .. })));
        assert!(matches!(Call::parse(Method::Get, &format!("/v1/sites/{SITE}/nope")), Err(RouteError::NoRoute { .. })));
        assert!(matches!(Call::parse(Method::Get, "/v1/sites/xyz"), Err(RouteError::Id { .. })));
        let bad = format!("/v1/sites/{SITE}/deploys/zz/export");
        assert!(matches!(Call::parse(Method::Get, &bad), Err(RouteError::Deploy { .. })));
    }
}
