//! The failures the dashboard has, typed: what failed, with the cause chain kept as a chain.

use std::error::Error;
use std::fmt;

use build::BuildId;
use control::{Handle, Refusal};

use crate::form::Field;

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
    /// A session segment that is no session id.
    Session(control::IdError),
}

impl fmt::Display for Unrouted {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path => out.write_str("no screen has this address"),
            Self::Handle(_) => out.write_str("the first part of the address is not a handle"),
            Self::Build(_) => out.write_str("the build in the address is not a build id"),
            Self::Code(_) => out.write_str("the decision in the address is not a decision code"),
            Self::File(_) => out.write_str("the file in the address is not a path"),
            Self::Session(_) => out.write_str("the session in the address is not a session id"),
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
            Self::Session(source) => Some(source),
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
    /// The sign-in routes of the server refused, and said why.
    Auth {
        /// The status they answered with.
        status: crate::Status,
        /// Their stable code, such as `auth.origin.refused`.
        problem: String,
        /// What they said, and why.
        source: Cause,
    },
    /// The site's source is an upload, which only `ink push` can change.
    Upload {
        /// The site's handle.
        handle: Handle,
    },
    /// The DNS lookup of a proof record got no answer.
    Lookup {
        /// The name being proved.
        name: String,
        /// Why, as a chain.
        source: Cause,
    },
    /// The replay of a decision runs where the source is, which this server does not hold.
    NoReplay,
}

/// Why a form field does not hold what it should.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldError {
    /// Not a deploy id.
    Deploy(model::HashError),
    /// Not a handle.
    Handle(control::HandleError),
    /// Not a domain name.
    Domain(control::NameError),
}

impl fmt::Display for FieldError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deploy(_) => out.write_str("not a deploy id"),
            Self::Handle(_) => out.write_str("not a handle"),
            Self::Domain(_) => out.write_str("not a domain name"),
        }
    }
}

impl Error for FieldError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Deploy(source) => Some(source),
            Self::Handle(source) => Some(source),
            Self::Domain(source) => Some(source),
        }
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
            Self::Auth { problem, .. } => write!(out, "signing in was refused: {problem}"),
            Self::Upload { handle } => {
                write!(out, "{handle} is built from an upload, so there is nothing to read again")
            }
            Self::Lookup { name, .. } => write!(out, "the proof record of {name} could not be looked up"),
            Self::NoReplay => out.write_str("this server cannot replay a decision"),
        }
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unrouted { source } => Some(source),
            Self::Invalid { source, .. } => Some(source),
            Self::Refused { refusal } => Some(refusal),
            Self::Store { source, .. }
            | Self::Render { source }
            | Self::Auth { source, .. }
            | Self::Lookup { source, .. } => Some(source),
            _ => None,
        }
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
