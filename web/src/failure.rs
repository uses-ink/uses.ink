//! What the dashboard fails with: typed, with a stable code, a cause chain and a fix (decision
//! 0053). A failure is a page too, so every screen has an error state.

use std::error::Error;
use std::fmt;

use build::BuildId;
use control::{Handle, Refusal};

use crate::Status;
use crate::form::Field;

/// The stable name of a failure: `noun.noun.reason`, as every code in uses.ink is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Code {
    /// `page.route.unknown`.
    RouteUnknown,
    /// `page.method.refused`.
    MethodRefused,
    /// `account.session.missing`.
    SessionMissing,
    /// `account.site.foreign`.
    SiteForeign,
    /// `site.handle.unknown`.
    HandleUnknown,
    /// `build.id.unknown`.
    BuildUnknown,
    /// `form.field.missing`.
    FieldMissing,
    /// `form.field.invalid`.
    FieldInvalid,
    /// `form.confirm.mismatch`.
    ConfirmMismatch,
    /// `control.request.refused`.
    RequestRefused,
    /// `control.store.failed`.
    StoreFailed,
    /// `build.log.cut`.
    LogCut,
    /// `page.render.failed`.
    RenderFailed,
}

impl Code {
    /// The code as written.
    pub const fn name(self) -> &'static str {
        match self {
            Self::RouteUnknown => "page.route.unknown",
            Self::MethodRefused => "page.method.refused",
            Self::SessionMissing => "account.session.missing",
            Self::SiteForeign => "account.site.foreign",
            Self::HandleUnknown => "site.handle.unknown",
            Self::BuildUnknown => "build.id.unknown",
            Self::FieldMissing => "form.field.missing",
            Self::FieldInvalid => "form.field.invalid",
            Self::ConfirmMismatch => "form.confirm.mismatch",
            Self::RequestRefused => "control.request.refused",
            Self::StoreFailed => "control.store.failed",
            Self::LogCut => "build.log.cut",
            Self::RenderFailed => "page.render.failed",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// What kind of page a path was taken for, when the path named none.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unrouted {
    /// A path of no screen.
    Path,
    /// A handle segment that is no handle.
    Handle(control::HandleError),
    /// A build segment that is no build id.
    Build(build::BuildIdError),
    /// A decision code that is none.
    Code(model::UnknownDecision),
    /// A file path that is none.
    File(model::PathError),
}

impl fmt::Display for Unrouted {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path => out.write_str("no screen has this address"),
            Self::Handle(_) => out.write_str("the first part of the address is not a handle"),
            Self::Build(_) => out.write_str("the build in the address is not a build id"),
            Self::Code(_) => out.write_str("the decision in the address is not a decision code"),
            Self::File(_) => out.write_str("the file in the address is not a path"),
        }
    }
}

impl Error for Unrouted {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Path => None,
            Self::Handle(source) => Some(source),
            Self::Build(source) => Some(source),
            Self::Code(source) => Some(source),
            Self::File(source) => Some(source),
        }
    }
}

/// A part of the control plane's store that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Part {
    /// The database.
    Database,
    /// The blob store.
    Blobs,
    /// The queue.
    Queue,
    /// The sealer of credentials.
    Secrets,
    /// A stored row, manifest or log that does not read back.
    Records,
}

impl fmt::Display for Part {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Database => "the database",
            Self::Blobs => "the blob store",
            Self::Queue => "the queue",
            Self::Secrets => "the credential sealer",
            Self::Records => "the stored records",
        })
    }
}

/// A chain of causes kept as a chain: each link says what failed, the next says why. It is made
/// once, at the edge, from the failure of a port whose types the dashboard does not name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cause {
    said: String,
    next: Option<Box<Cause>>,
}

impl Cause {
    /// The chain that starts at `error`, followed through its `source()`.
    pub fn of(error: &(dyn Error + 'static)) -> Self {
        Self { said: error.to_string(), next: error.source().map(|next| Box::new(Self::of(next))) }
    }
}

impl fmt::Display for Cause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.said)
    }
}

impl Error for Cause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.next.as_deref().map(|next| next as &(dyn Error + 'static))
    }
}

/// A request the dashboard could not answer with the page that was asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Failure {
    /// The address names no screen.
    Unrouted {
        /// Why.
        source: Unrouted,
    },
    /// The screen does not take this method.
    Method {
        /// The method that was used.
        used: crate::Method,
    },
    /// Nobody is signed in.
    SignedOut,
    /// The site belongs to another account.
    Foreign {
        /// The site's handle.
        handle: Handle,
    },
    /// No site has the handle.
    NoSite {
        /// The handle.
        handle: Handle,
    },
    /// The site has no such build.
    NoBuild {
        /// The build.
        build: BuildId,
    },
    /// A form field is missing.
    Missing {
        /// Which.
        field: Field,
    },
    /// A form field does not hold what it should.
    Invalid {
        /// Which.
        field: Field,
        /// Why.
        source: FieldError,
    },
    /// The handle typed to confirm a removal is not the site's.
    Confirm {
        /// The handle that was expected.
        expected: Handle,
    },
    /// The control plane refused, and said why.
    Refused {
        /// What it said.
        refusal: Refusal,
    },
    /// A part of the store failed.
    Store {
        /// Which part.
        part: Part,
        /// What it failed with.
        source: Cause,
    },
    /// The live log ended before the build did.
    Cut {
        /// The build.
        build: BuildId,
    },
    /// A page could not be written out.
    Render {
        /// What the sink or the split said.
        source: Cause,
    },
}

/// Why a form field does not hold what it should.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldError {
    /// Not a deploy id.
    Deploy(model::HashError),
    /// Not a handle.
    Handle(control::HandleError),
}

impl fmt::Display for FieldError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deploy(_) => out.write_str("not a deploy id"),
            Self::Handle(_) => out.write_str("not a handle"),
        }
    }
}

impl Error for FieldError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Deploy(source) => Some(source),
            Self::Handle(source) => Some(source),
        }
    }
}

impl Failure {
    /// Its stable code.
    pub const fn code(&self) -> Code {
        match self {
            Self::Unrouted { .. } => Code::RouteUnknown,
            Self::Method { .. } => Code::MethodRefused,
            Self::SignedOut => Code::SessionMissing,
            Self::Foreign { .. } => Code::SiteForeign,
            Self::NoSite { .. } => Code::HandleUnknown,
            Self::NoBuild { .. } => Code::BuildUnknown,
            Self::Missing { .. } => Code::FieldMissing,
            Self::Invalid { .. } => Code::FieldInvalid,
            Self::Confirm { .. } => Code::ConfirmMismatch,
            Self::Refused { .. } => Code::RequestRefused,
            Self::Store { .. } => Code::StoreFailed,
            Self::Cut { .. } => Code::LogCut,
            Self::Render { .. } => Code::RenderFailed,
        }
    }

    /// The status its page is sent with.
    pub const fn status(&self) -> Status {
        match self {
            Self::Unrouted { .. } | Self::NoSite { .. } | Self::NoBuild { .. } => Status::NotFound,
            Self::Method { .. } => Status::MethodNotAllowed,
            Self::SignedOut => Status::Unauthorized,
            Self::Foreign { .. } => Status::Forbidden,
            Self::Missing { .. } | Self::Invalid { .. } | Self::Confirm { .. } => Status::BadRequest,
            Self::Refused { .. } => Status::Conflict,
            Self::Store { .. } | Self::Cut { .. } | Self::Render { .. } => Status::Internal,
        }
    }

    /// What to do about it, in a sentence of its own.
    pub fn fix(&self) -> Fix<'_> {
        Fix(self)
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unrouted { source } => source.fmt(out),
            Self::Method { used } => write!(out, "this page does not take {used}"),
            Self::SignedOut => out.write_str("you are not signed in"),
            Self::Foreign { handle } => write!(out, "{handle} belongs to another account"),
            Self::NoSite { handle } => write!(out, "no site has the handle {handle}"),
            Self::NoBuild { build } => write!(out, "this site has no build {build}"),
            Self::Missing { field } => write!(out, "the form has no {field}"),
            Self::Invalid { field, source } => write!(out, "the {field} is {source}"),
            Self::Confirm { expected } => write!(out, "the name typed is not {expected}, so nothing was removed"),
            Self::Refused { refusal } => refusal.fmt(out),
            Self::Store { part, .. } => write!(out, "{part} failed"),
            Self::Cut { build } => write!(out, "the live log of build {build} ended before the build did"),
            Self::Render { .. } => out.write_str("a page could not be written out"),
        }
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unrouted { source } => Some(source),
            Self::Invalid { source, .. } => Some(source),
            Self::Refused { refusal } => Some(refusal),
            Self::Store { source, .. } | Self::Render { source } => Some(source),
            _ => None,
        }
    }
}

/// The fix for a failure, worded.
#[derive(Clone, Copy, Debug)]
pub struct Fix<'a>(&'a Failure);

impl fmt::Display for Fix<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Failure::Unrouted { .. } => out.write_str("use the links on the page; the five screens are Site, What we understood, Deploys, Settings and Account"),
            Failure::Method { .. } => out.write_str("open the page by its link, or send the form from the page it belongs to"),
            Failure::SignedOut => out.write_str("sign in again; your session has ended or never began"),
            Failure::Foreign { .. } => out.write_str("sign in with the account that owns the site"),
            Failure::NoSite { .. } => out.write_str("check the handle in the address; your own site is listed under Account"),
            Failure::NoBuild { .. } => out.write_str("open the build from the Deploys screen"),
            Failure::Missing { .. } | Failure::Invalid { .. } => {
                out.write_str("send the form from its page, without changing its fields")
            }
            Failure::Confirm { .. } => out.write_str("type the site's handle exactly, or leave the site as it is"),
            Failure::Refused { refusal } => Refused(refusal).fmt(out),
            Failure::Store { .. } => out.write_str("try again in a minute; if it fails again, give the operator this code and the cause below"),
            Failure::Cut { .. } => out.write_str("reload the page: a finished build shows its whole log"),
            Failure::Render { .. } => out.write_str("reload the page; if it fails again, give the operator this code and the cause below"),
        }
    }
}

/// What to do about each thing the control plane refuses.
#[derive(Clone, Copy, Debug)]
struct Refused<'a>(&'a Refusal);

impl fmt::Display for Refused<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Refusal::Reserved { .. } | Refusal::Taken { .. } | Refusal::Tombstoned { .. } => "choose another handle",
            Refusal::AlreadyLive { .. } => "nothing to do: this deploy is the one being served",
            Refusal::NoDeploy { .. } | Refusal::NotLive { .. } => "open Deploys and pick a deploy the site has",
            Refusal::NoSite { .. } | Refusal::Removing { .. } => {
                "the site is gone or going; open Account to see your sites"
            }
            Refusal::MissingBlob { .. } | Refusal::NoManifest { .. } => {
                "build the site again; its stored files are incomplete"
            }
            _ => "reload the page and try again; if it repeats, give the operator this code",
        })
    }
}

impl<P: control::Ports + 'static> From<control::Error<P>> for Failure {
    fn from(error: control::Error<P>) -> Self {
        let part = match &error {
            control::Error::Refused(refusal) => return Self::Refused { refusal: refusal.clone() },
            control::Error::Db(_) | control::Error::Statement(_) => Part::Database,
            control::Error::Blob(_) => Part::Blobs,
            control::Error::Queue(_) => Part::Queue,
            control::Error::Secret(_) => Part::Secrets,
            _ => Part::Records,
        };
        Self::Store { part, source: Cause::of(&error) }
    }
}
