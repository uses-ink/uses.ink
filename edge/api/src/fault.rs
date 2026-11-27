//! Why a call failed, and what it is answered with.

use std::error::Error;
use std::fmt;

use control::{Failed, Ports, Refusal};
use model::Hash;

use crate::body::Unreadable;
use crate::credential::BearerError;
use crate::limits::Raised;
use crate::reply::{Media, Reply, Status};
use crate::route::RouteError;
use crate::tar;
use crate::view::Problem;

/// A request that does not name a caller who may make it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Denied {
    /// No `Authorization` header.
    Missing,
    /// A header that is no bearer token.
    Malformed(BearerError),
    /// A token that speaks for nobody.
    Unknown,
}

impl fmt::Display for Denied {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => out.write_str("send the owner token as `Authorization: Bearer <token>`"),
            Self::Malformed(_) => out.write_str("the Authorization header is not a Bearer token"),
            Self::Unknown => out.write_str("that token speaks for nobody"),
        }
    }
}

impl Error for Denied {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(source) => Some(source),
            _ => None,
        }
    }
}

/// An archive that is well formed and is no built folder.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Intake {
    /// The archive holds no `.ink-manifest.json`.
    NoManifest,
    /// The manifest lists a file the archive does not hold.
    Absent {
        /// The path.
        path: String,
    },
    /// A file's bytes are not the hash the manifest lists for it.
    Mismatch {
        /// The path.
        path: String,
        /// What the manifest says.
        listed: Hash,
        /// What the bytes hash to.
        found: Hash,
    },
}

impl fmt::Display for Intake {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoManifest => out.write_str("the archive holds no .ink-manifest.json: send what `ink build` wrote"),
            Self::Absent { path } => write!(out, "the manifest lists {path}, which the archive does not hold"),
            Self::Mismatch { path, listed, found } => {
                write!(out, "{path} hashes to {found}, but the manifest lists {listed}")
            }
        }
    }
}

impl Error for Intake {}

/// Every way a call fails. `P` is the control plane's ports.
#[non_exhaustive]
pub enum Fault<P: Ports> {
    /// The path is no route.
    Route(RouteError),
    /// The caller is not known.
    Denied(Denied),
    /// The credential port could not check the token.
    Credentials(Box<dyn Error>),
    /// The draw port gave no bytes.
    Draw(Box<dyn Error>),
    /// The body is not the JSON the route reads.
    Body(Unreadable),
    /// The body is over its limit.
    Large {
        /// The limit it went over.
        limit: u64,
    },
    /// A git source with a scheme that is not `https`.
    Scheme {
        /// The URL as sent.
        url: String,
    },
    /// A site whose content nothing builds.
    Unbuilt,
    /// A build that has written no log yet.
    NoLog,
    /// The control plane refused or failed.
    Control(control::Error<P>),
    /// An export failed.
    Export(Failed<P, tar::Error>),
    /// The archive cannot be read.
    Archive(tar::Error),
    /// The archive's manifest is not a deploy.
    Folder(edge_folder::Error),
    /// The archive is no built folder.
    Intake(Intake),
    /// A limit set lower than the operator's is above it.
    Raised(Raised),
}

impl<P: Ports> fmt::Debug for Fault<P> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Fault({})", self.name())
    }
}

impl<P: Ports> fmt::Display for Fault<P> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route(error) => error.fmt(out),
            Self::Denied(error) => error.fmt(out),
            Self::Credentials(_) => out.write_str("the token could not be checked"),
            Self::Draw(_) => out.write_str("no random bytes were available"),
            Self::Body(error) => error.fmt(out),
            Self::Large { limit } => write!(out, "the body is over the limit of {limit} bytes"),
            Self::Scheme { .. } => out.write_str("a git source is fetched over https"),
            Self::Unbuilt => out.write_str("nothing builds this site: it takes uploads"),
            Self::NoLog => out.write_str("the build has written no log yet"),
            Self::Control(error) => error.fmt(out),
            Self::Export(error) => error.fmt(out),
            Self::Archive(error) => error.fmt(out),
            Self::Folder(error) => error.fmt(out),
            Self::Intake(error) => error.fmt(out),
            Self::Raised(error) => error.fmt(out),
        }
    }
}

impl<P: Ports + 'static> Error for Fault<P> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Route(inner) => inner.source(),
            Self::Denied(inner) => inner.source(),
            Self::Credentials(source) | Self::Draw(source) => Some(source.as_ref()),
            Self::Body(source) => Some(source),
            Self::Control(inner) => inner.source(),
            Self::Export(inner) => inner.source(),
            Self::Archive(inner) => inner.source(),
            Self::Folder(inner) => inner.source(),
            Self::Raised(inner) => inner.source(),
            _ => None,
        }
    }
}

impl<P: Ports> From<control::Error<P>> for Fault<P> {
    fn from(error: control::Error<P>) -> Self {
        Self::Control(error)
    }
}

impl<P: Ports> From<Intake> for Fault<P> {
    fn from(error: Intake) -> Self {
        Self::Intake(error)
    }
}

impl<P: Ports> From<Refusal> for Fault<P> {
    fn from(refusal: Refusal) -> Self {
        Self::Control(refusal.into())
    }
}

impl<P: Ports> Fault<P> {
    /// A stable name for what went wrong.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Route(_) => "route",
            Self::Denied(_) => "unauthorized",
            Self::Credentials(_) => "credentials",
            Self::Draw(_) => "random",
            Self::Body(_) => "body",
            Self::Large { .. } => "too-large",
            Self::Scheme { .. } => "scheme",
            Self::Unbuilt => "unbuilt",
            Self::NoLog => "no-log",
            Self::Control(_) => "control",
            Self::Export(_) => "export",
            Self::Archive(_) => "archive",
            Self::Folder(_) => "manifest",
            Self::Intake(_) => "intake",
            Self::Raised(_) => "raised",
        }
    }

    /// The status this is answered with.
    pub fn status(&self) -> Status {
        match self {
            Self::Route(RouteError::NoRoute { .. }) | Self::NoLog => Status::NotFound,
            Self::Route(RouteError::Method { .. }) => Status::MethodNotAllowed,
            Self::Denied(_) => Status::Unauthorized,
            Self::Large { .. }
            | Self::Archive(tar::Error::Files { .. } | tar::Error::File { .. } | tar::Error::Total { .. }) => {
                Status::TooLarge
            }
            Self::Route(_) | Self::Body(_) | Self::Archive(_) => Status::BadRequest,
            Self::Scheme { .. } | Self::Unbuilt | Self::Folder(_) | Self::Intake(_) | Self::Raised(_) => {
                Status::Unprocessable
            }
            Self::Control(error) => error.refusal().map_or(Status::Internal, Self::refused),
            Self::Credentials(_) | Self::Draw(_) | Self::Export(_) => Status::Internal,
        }
    }

    /// The status of a refusal of the control plane.
    const fn refused(refusal: &Refusal) -> Status {
        match refusal {
            Refusal::Reserved { .. }
            | Refusal::Taken { .. }
            | Refusal::Tombstoned { .. }
            | Refusal::Exists { .. }
            | Refusal::Removing { .. }
            | Refusal::NotLive { .. }
            | Refusal::AlreadyLive { .. } => Status::Conflict,
            Refusal::NoSite { .. }
            | Refusal::NoDeploy { .. }
            | Refusal::NoBuild { .. }
            | Refusal::NoManifest { .. }
            | Refusal::NoCredential { .. } => Status::NotFound,
            Refusal::NoAccount { .. } | Refusal::MissingBlob { .. } => Status::Unprocessable,
            _ => Status::Internal,
        }
    }
}

impl<P: Ports + 'static> Fault<P> {
    /// The answer: a status and a [`Problem`] that says what is wrong and, for what the caller
    /// did, each cause below it. A failure of the server's own gives its message and no causes.
    pub fn reply(&self) -> Reply {
        let status = self.status();
        let mut causes = Vec::new();
        let mut below = self.source();
        while let Some(cause) = below.filter(|_| status != Status::Internal) {
            causes.push(cause.to_string());
            below = cause.source();
        }
        let problem = Problem { problem: self.name(), message: self.to_string(), causes };
        let body = serde_json::to_vec(&problem).unwrap_or_default();
        let mut reply = Reply::new(status, Media::Json, body);
        reply.challenge = matches!(self, Self::Denied(_));
        if let Self::Route(RouteError::Method { allowed, .. }) = self {
            reply.allow.clone_from(allowed);
        }
        reply
    }
}
