//! Why a call failed, and what it is answered with.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use control::keys::{Refused, Right};
use control::{Failed, Ports, Refusal};
use db::Db;
use jiff::Timestamp;
use model::Hash;
use model::git::Scheme;

use crate::archive;
use crate::body::Unreadable;
use crate::enrol::Closed;
use crate::limits::Raised;
use crate::port::{Port, Unnamed};
use crate::refusal::Turned;
use crate::reply::{Media, Reply, Status};
use crate::route::RouteError;
use crate::source::Sourced;
use crate::view::Problem;

/// A request that does not name a caller who may make it. Each is one code of the catalog
/// (`auth.*`), one status and one body shape; [`Denied::Refused`] is the one answer for a
/// signature that does not verify, whatever the reason, so a caller learns nothing from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Denied {
    /// No `Authorization` header.
    Missing,
    /// A header that is not the `Ink-Sig-1` profile.
    Malformed,
    /// A request target that is not in canonical form.
    NotCanonical,
    /// The request's time is outside the window.
    Skewed {
        /// How many seconds from the server's time.
        skew: u64,
        /// How many are allowed.
        window: u64,
        /// What time the server thinks it is.
        server_time: Timestamp,
    },
    /// Unknown key, wrong signature or wrong audience: one answer.
    Refused,
    /// A valid signature of a revoked key.
    Revoked {
        /// When it was revoked.
        since: Timestamp,
    },
    /// A valid signature of a key past its end.
    Expired {
        /// When it ended.
        since: Timestamp,
    },
    /// A valid signature whose nonce was seen.
    Replayed,
    /// A key with as many nonces remembered as are kept at once.
    Flooded {
        /// The window, in seconds.
        window: u64,
    },
    /// The body is not the one that was signed.
    Digest,
    /// The server started a moment ago and takes signed requests again soon.
    Restarting {
        /// How many seconds.
        seconds: u64,
    },
    /// The key lacks a right.
    Right {
        /// The right.
        right: Right,
        /// Where it was needed.
        scope: &'static str,
    },
    /// Enrolling is closed for a while.
    Limited(Closed),
}

impl Denied {
    /// The catalog code of the answer.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Missing | Self::Malformed => "auth.signature.malformed",
            Self::NotCanonical => "auth.target.not-canonical",
            Self::Skewed { .. } => "auth.signature.skewed",
            Self::Refused => "auth.signature.refused",
            Self::Revoked { .. } => "auth.key.revoked",
            Self::Expired { .. } => "auth.key.expired",
            Self::Replayed => "auth.signature.replayed",
            Self::Flooded { .. } => "auth.signature.flooded",
            Self::Digest => "auth.digest.mismatch",
            Self::Restarting { .. } => "auth.server.restarting",
            Self::Right { .. } => "auth.right.missing",
            Self::Limited(_) => "auth.enrolment.limited",
        }
    }

    /// The status.
    pub const fn status(&self) -> Status {
        match self {
            Self::Malformed | Self::Missing | Self::NotCanonical | Self::Digest => Status::BadRequest,
            Self::Right { .. } => Status::Forbidden,
            Self::Flooded { .. } | Self::Limited(_) => Status::TooMany,
            Self::Restarting { .. } => Status::Unavailable,
            _ => Status::Unauthorized,
        }
    }

    /// The data of the answer, named as the catalog names its fields.
    pub fn fields(&self) -> BTreeMap<&'static str, String> {
        let mut fields = BTreeMap::new();
        match self {
            Self::Skewed { skew, window, server_time } => {
                fields.insert("skew", skew.to_string());
                fields.insert("window", window.to_string());
                fields.insert("server_time", server_time.to_string());
            }
            Self::Revoked { since } | Self::Expired { since } => {
                fields.insert("since", since.to_string());
            }
            Self::Flooded { window } => {
                fields.insert("window", window.to_string());
            }
            Self::Restarting { seconds } => {
                fields.insert("seconds", seconds.to_string());
            }
            Self::Limited(closed) => {
                fields.insert("seconds", closed.seconds.to_string());
            }
            Self::Right { right, scope } => {
                fields.insert("right", right.name().to_owned());
                fields.insert("scope", (*scope).to_owned());
            }
            _ => {}
        }
        fields
    }
}

impl fmt::Display for Denied {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => out.write_str("sign the request: `Authorization: Ink-Sig-1 ...`"),
            Self::Malformed => out.write_str("the Authorization header is not an Ink-Sig-1 value"),
            Self::NotCanonical => out.write_str("the request target is not in canonical form"),
            Self::Skewed { skew, window, .. } => {
                write!(out, "the request's time is {skew} seconds from the server's, over the allowed {window}")
            }
            Self::Refused => out.write_str("the server did not accept the signature"),
            Self::Revoked { since } => write!(out, "this key was revoked on {since}"),
            Self::Expired { since } => write!(out, "this key expired on {since}"),
            Self::Replayed => out.write_str("a request with the same nonce was already seen"),
            Self::Flooded { window } => {
                write!(out, "this key sent more signed requests than are remembered in {window} seconds")
            }
            Self::Digest => out.write_str("the body is not the one that was signed"),
            Self::Restarting { seconds } => {
                write!(out, "the server just started and takes signed requests in {seconds} seconds")
            }
            Self::Right { right, scope } => {
                write!(out, "this key has no `{right}` right on the {scope}", right = right.name())
            }
            Self::Limited(closed) => closed.fmt(out),
        }
    }
}

impl Error for Denied {}

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
    /// A port failed: the credential check or the random source.
    Port(Unnamed),
    /// The key store failed.
    Store(control::keys::Error<<P::Db as Db>::Error>),
    /// The key store refused, for a reason about the request.
    Keys(Refused),
    /// The person turned the ask down.
    Declined,
    /// The body is not the JSON the route reads.
    Body(Unreadable),
    /// The body is over its limit.
    Large {
        /// The limit it went over.
        limit: u64,
    },
    /// A git source with a scheme this server does not fetch over.
    Scheme {
        /// The scheme of the address as sent.
        scheme: Scheme,
        /// The schemes the operator allows.
        allowed: Vec<Scheme>,
    },
    /// A site whose content nothing builds.
    Unbuilt,
    /// A build that has written no log yet.
    NoLog,
    /// The control plane refused or failed.
    Control(control::Error<P>),
    /// An export failed.
    Export(Failed<P, archive::Error>),
    /// The archive cannot be read.
    Archive(archive::Error),
    /// The archive's manifest is not a deploy.
    Folder(edge_folder::Error),
    /// The archive is no built folder.
    Intake(Intake),
    /// A limit set lower than the operator's is above it.
    Raised(Raised),
    /// An upload of source is refused.
    Source(Sourced),
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
            Self::Port(error) => error.fmt(out),
            Self::Store(_) => out.write_str("the credential could not be checked"),
            Self::Keys(error) => error.fmt(out),
            Self::Declined => out.write_str("the request was not approved"),
            Self::Body(error) => error.fmt(out),
            Self::Large { limit } => write!(out, "the body is over the limit of {limit} bytes"),
            Self::Scheme { scheme, allowed } => {
                let allowed: Vec<&str> = allowed.iter().map(|scheme| scheme.name()).collect();
                write!(out, "this server fetches git sources over {}, not {scheme}", allowed.join(" or "))
            }
            Self::Unbuilt => out.write_str("nothing builds this site: it takes uploads"),
            Self::NoLog => out.write_str("the build has written no log yet"),
            Self::Control(error) => error.fmt(out),
            Self::Export(error) => error.fmt(out),
            Self::Archive(error) => error.fmt(out),
            Self::Folder(error) => error.fmt(out),
            Self::Intake(error) => error.fmt(out),
            Self::Raised(error) => error.fmt(out),
            Self::Source(error) => error.fmt(out),
        }
    }
}

impl<P: Ports + 'static> Error for Fault<P> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Route(inner) => inner.source(),
            Self::Denied(inner) => inner.source(),
            Self::Keys(inner) => Some(inner),
            Self::Port(source) => Some(source),
            Self::Store(source) => Some(source),
            Self::Body(source) => Some(source),
            Self::Control(inner) => inner.source(),
            Self::Export(inner) => inner.source(),
            Self::Archive(inner) => inner.source(),
            Self::Folder(inner) => inner.source(),
            Self::Raised(inner) => inner.source(),
            Self::Source(inner) => Some(inner),
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

impl<P: Ports> From<Refused> for Fault<P> {
    fn from(refused: Refused) -> Self {
        Self::Keys(refused)
    }
}

impl<P: Ports> From<Closed> for Fault<P> {
    fn from(closed: Closed) -> Self {
        Self::Denied(Denied::Limited(closed))
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
            Self::Denied(denied) => denied.code(),
            Self::Port(port) => match port.port() {
                Port::Draw => "random",
                _ => "credentials",
            },
            Self::Store(_) => "credentials",
            Self::Keys(refused) => Turned(refused).code(),
            Self::Declined => "auth.enrolment.denied",
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
            Self::Source(error) => error.name(),
        }
    }

    /// The status this is answered with.
    pub fn status(&self) -> Status {
        match self {
            Self::Route(RouteError::NoRoute { .. }) | Self::NoLog => Status::NotFound,
            Self::Route(RouteError::Method { .. }) => Status::MethodNotAllowed,
            Self::Denied(denied) => denied.status(),
            Self::Keys(refused) => Turned(refused).status(),
            Self::Declined => Status::Forbidden,
            Self::Large { .. }
            | Self::Archive(
                archive::Error::Files { .. } | archive::Error::File { .. } | archive::Error::Total { .. },
            ) => Status::TooLarge,
            Self::Route(_) | Self::Body(_) | Self::Archive(_) => Status::BadRequest,
            Self::Scheme { .. } | Self::Unbuilt | Self::Folder(_) | Self::Intake(_) | Self::Raised(_) => {
                Status::Unprocessable
            }
            Self::Source(error) => error.status(),
            Self::Control(error) => error.refusal().map_or(Status::Internal, Self::refused),
            Self::Port(_) | Self::Store(_) | Self::Export(_) => Status::Internal,
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
        let fields = match self {
            Self::Denied(denied) => denied.fields(),
            _ => BTreeMap::new(),
        };
        let problem = Problem { problem: self.name(), message: self.to_string(), causes, fields };
        let body = serde_json::to_vec(&problem).unwrap_or_default();
        let mut reply = Reply::new(status, Media::Json, body);
        reply.challenge = matches!(self, Self::Denied(_));
        reply.retry = match self {
            Self::Denied(Denied::Restarting { seconds }) => Some(Duration::from_secs(*seconds)),
            Self::Denied(Denied::Limited(closed)) => Some(Duration::from_secs(closed.seconds)),
            _ => None,
        };
        if let Self::Route(RouteError::Method { allowed, .. }) = self {
            reply.allow.clone_from(allowed);
        }
        reply
    }
}

impl trace::Kind for &Denied {
    /// Why the caller was turned away, never the token.
    fn kind(self) -> &'static str {
        match self {
            Denied::Missing | Denied::Malformed | Denied::NotCanonical => "malformed",
            Denied::Skewed { .. } => "skewed",
            Denied::Refused => "refused",
            Denied::Revoked { .. } => "revoked",
            Denied::Expired { .. } => "expired",
            Denied::Replayed => "replayed",
            Denied::Flooded { .. } => "flooded",
            Denied::Digest => "digest",
            Denied::Restarting { .. } => "restarting",
            Denied::Right { .. } => "right",
            Denied::Limited(_) => "limited",
        }
    }
}
