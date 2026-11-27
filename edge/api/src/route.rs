//! The API's routes: a method and a path become one [`Call`], or say why they are none.

use std::fmt;

use build::BuildId;
use control::keys::Right;
use control::{EnrolmentKind, Id, IdError, SiteKind};
use model::DeployId;
use serde::Deserialize;
use serde::de::IntoDeserializer;
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
    /// `PUT /v1/sites/{site}/deploy`: a built folder as a tar archive, made a deploy.
    Intake(Id<SiteKind>),
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
    },
    /// A part of the path should be a build and is not.
    Build {
        /// The text.
        written: String,
    },
    /// A part of the path should be a deploy and is not.
    Deploy {
        /// The text.
        written: String,
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
            Self::Fingerprint { written } => write!(out, "{written:?} in the path is not a key fingerprint"),
            Self::Build { written } => write!(out, "{written:?} in the path is not a build id"),
            Self::Deploy { written } => write!(out, "{written:?} in the path is not a deploy id"),
        }
    }
}

impl std::error::Error for RouteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Id { source, .. } => Some(source),
            _ => None,
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
                Err(_) => Err(RouteError::Fingerprint { written: (*print).to_owned() }),
            },
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
            ["builds"] => by(&[(Method::Get, Self::Builds(site)), (Method::Post, Self::Build(site))]),
            ["builds", build, "log"] => match build.parse::<BuildId>() {
                Ok(build) => by(&[(Method::Get, Self::Log { site, build })]),
                Err(_) => Err(RouteError::Build { written: (*build).to_owned() }),
            },
            ["export"] => by(&[(Method::Get, Self::Export { site, deploy: None })]),
            ["deploy"] => by(&[(Method::Put, Self::Intake(site))]),
            _ => return None,
        })
    }

    /// The right a call needs and the site it is about, when it is about one. `None` for the
    /// calls that need none: asking to enrol, polling one's own ask, and knowing who one is.
    pub const fn needs(&self) -> Option<(Right, Option<Id<SiteKind>>)> {
        Some(match self {
            Self::Sites => (Right::Read, None),
            Self::Add => (Right::Edit, None),
            Self::Status(site)
            | Self::Deploys(site)
            | Self::Builds(site)
            | Self::Log { site, .. }
            | Self::Export { site, .. } => (Right::Read, Some(*site)),
            Self::Remove(site) => (Right::Destroy, Some(*site)),
            Self::Rollback(site) | Self::Resume(site) | Self::Build(site) | Self::Intake(site) => {
                (Right::Deploy, Some(*site))
            }
            Self::Keys | Self::Revoke(_) => (Right::Keys, None),
            Self::Enrol | Self::Polled(_) | Self::Whoami => return None,
        })
    }

    fn site(text: &str) -> Result<Id<SiteKind>, RouteError> {
        text.parse().map_err(|source| RouteError::Id { noun: "site", source })
    }

    fn deploy(text: &str) -> Result<DeployId, RouteError> {
        DeployId::deserialize(text.into_deserializer())
            .map_err(|_: serde::de::value::Error| RouteError::Deploy { written: text.to_owned() })
    }
}

impl trace::Kind for &Call {
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
            Call::Enrol => "enrol",
            Call::Polled(_) => "polled",
            Call::Whoami => "whoami",
            Call::Keys => "keys",
            Call::Revoke(_) => "revoke",
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
