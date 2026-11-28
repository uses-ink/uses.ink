//! Why a passkey call was not answered with what it asked for, and how each is answered.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use control::keys::LabelError;
use control::passkeys::{Flow, Reason, Refusal, Refused as Stored};
use control::signin::Error as SignError;

use super::route::StepError;
use crate::auth::signin::Limited;
use crate::body::Unreadable;
use crate::reply::{Media, Reply, Status};
use crate::view::Problem;

/// A passkey call that did not succeed. A refusal of a ceremony says which rule it broke, as a
/// catalog code of its own; a failure of the server's own gives a message and no causes.
#[derive(Debug)]
#[non_exhaustive]
pub enum Stop {
    /// The path is no passkey route.
    Route,
    /// The method is not taken there.
    Method(StepError),
    /// The body is not the JSON the route reads.
    Body(Unreadable),
    /// The body went over its limit.
    Large {
        /// The limit.
        limit: u64,
    },
    /// The request did not come from one of the dashboard's origins.
    Origin,
    /// A rate limit was reached.
    Limited(Limited),
    /// The request holds no usable session.
    Anonymous,
    /// There is no ceremony to complete: it ended, was already used, was never started, or is
    /// another browser's. All four look the same.
    Challenge,
    /// A reply was refused.
    Refused {
        /// Which ceremony it was for.
        flow: Flow,
        /// What was wrong.
        refusal: Refusal,
    },
    /// The credential is already registered.
    Taken,
    /// The account has no such passkey.
    Missing,
    /// The label is not a label.
    Label(LabelError),
    /// The server is set up for another relying party id than the passkeys were made for.
    Moved {
        /// The id the first passkey was made under.
        was: String,
        /// The id now configured.
        now: String,
        /// How many passkeys would stop working.
        passkeys: u64,
    },
    /// Another sign-in with the same passkey wrote its counter first.
    Raced,
    /// The stores failed.
    Store(Box<dyn Error>),
    /// The verifier failed.
    Verifier(Box<dyn Error>),
    /// The random source gave nothing.
    Draw(Box<dyn Error>),
    /// The answer could not be written as JSON.
    Json(serde_json::Error),
}

impl fmt::Display for Stop {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route => out.write_str("there is no such route"),
            Self::Method(error) => error.fmt(out),
            Self::Body(error) => error.fmt(out),
            Self::Large { limit } => write!(out, "the body is over the limit of {limit} bytes"),
            Self::Origin => out.write_str("the request did not come from this dashboard"),
            Self::Limited(error) => error.fmt(out),
            Self::Anonymous => out.write_str("sign in on the dashboard: this route reads the session cookie"),
            Self::Challenge => out.write_str("no passkey ceremony is waiting for this reply"),
            Self::Refused { refusal, .. } => refusal.fmt(out),
            Self::Taken => out.write_str("that passkey is already registered"),
            Self::Missing => out.write_str("the account has no passkey with that id"),
            Self::Label(error) => error.fmt(out),
            Self::Moved { was, now, passkeys } => write!(
                out,
                "passkeys were made for `{was}` and the relying party id is now `{now}`; {passkeys} would stop working"
            ),
            Self::Raced => out.write_str("the passkey was used at the same moment by another sign-in"),
            Self::Store(_) => out.write_str("the passkeys could not be read or written"),
            Self::Verifier(_) => out.write_str("the passkey verifier failed"),
            Self::Draw(_) => out.write_str("no random bytes were available"),
            Self::Json(_) => out.write_str("the answer could not be written"),
        }
    }
}

impl Error for Stop {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Method(source) => Some(source),
            Self::Body(source) => Some(source),
            Self::Refused { refusal, .. } => Some(refusal),
            Self::Label(source) => Some(source),
            Self::Limited(source) => Some(source),
            Self::Store(source) | Self::Verifier(source) | Self::Draw(source) => Some(source.as_ref()),
            Self::Json(source) => Some(source),
            _ => None,
        }
    }
}

impl Stop {
    /// The refusal of the passkey store this is, or a failure of it.
    pub fn stored<E: Error + 'static>(error: control::passkeys::Error<E>) -> Self {
        match error.refusal() {
            Some(Stored::Taken) => Self::Taken,
            Some(Stored::Missing) => Self::Missing,
            Some(Stored::Raced) => Self::Raced,
            Some(Stored::Moved { was, now, passkeys }) => {
                Self::Moved { was: was.clone(), now: now.clone(), passkeys: *passkeys }
            }
            _ => Self::Store(Box::new(error)),
        }
    }

    /// The failure of the sign-in store this is.
    pub fn signed<E: Error + 'static>(error: SignError<E>) -> Self {
        Self::Store(Box::new(error))
    }

    /// The catalog code, or the plain name of a failure that has none.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Route | Self::Method(_) => "route",
            Self::Body(_) => "body",
            Self::Large { .. } => "too-large",
            Self::Origin => "auth.origin.refused",
            Self::Limited(_) => "auth.rate.limited",
            Self::Anonymous => "unauthorized",
            Self::Challenge => "auth.passkey.challenge",
            Self::Refused { refusal, .. } => match refusal.reason {
                Reason::Origin => "auth.passkey.origin",
                Reason::RelyingParty => "auth.passkey.relying-party",
                Reason::Challenge => "auth.passkey.challenge",
                Reason::UserVerification => "auth.passkey.unverified",
                Reason::Counter => "auth.passkey.counter",
                Reason::Backup => "auth.passkey.backup",
                Reason::Malformed => "auth.passkey.malformed",
                Reason::Algorithm => "auth.passkey.algorithm",
                _ => "auth.passkey.refused",
            },
            Self::Taken => "auth.passkey.taken",
            Self::Missing => "auth.passkey.missing",
            Self::Label(_) => "auth.passkey.label",
            Self::Moved { .. } => "auth.relying-party.changed",
            Self::Raced => "auth.passkey.refused",
            Self::Store(_) => "passkeys",
            Self::Verifier(_) => "verifier",
            Self::Draw(_) => "random",
            Self::Json(_) => "json",
        }
    }

    /// The status.
    pub const fn status(&self) -> Status {
        match self {
            Self::Route | Self::Missing => Status::NotFound,
            Self::Method(_) => Status::MethodNotAllowed,
            Self::Body(_) | Self::Label(_) => Status::BadRequest,
            Self::Large { .. } => Status::TooLarge,
            Self::Origin => Status::Forbidden,
            Self::Limited(_) => Status::TooMany,
            Self::Anonymous | Self::Raced => Status::Unauthorized,
            Self::Challenge | Self::Taken | Self::Moved { .. } => Status::Conflict,
            Self::Refused { flow, .. } => match flow {
                Flow::Login => Status::Unauthorized,
                _ => Status::Unprocessable,
            },
            Self::Store(_) | Self::Verifier(_) | Self::Draw(_) | Self::Json(_) => Status::Internal,
        }
    }

    /// The answer: a status and a [`Problem`]. A failure of the server's own gives its message
    /// and no causes, and so does a refusal of a ceremony: what the verifier said is for the
    /// operator, not for whoever sent the reply.
    pub fn reply(&self) -> Reply {
        let status = self.status();
        let mut fields = BTreeMap::new();
        if let Self::Moved { was, now, passkeys } = self {
            fields.insert("was", was.clone());
            fields.insert("now", now.clone());
            fields.insert("passkeys", passkeys.to_string());
        }
        let mut causes = Vec::new();
        let hidden = matches!(self, Self::Refused { .. }) || status == Status::Internal;
        let mut below = Error::source(self).filter(|_| !hidden);
        while let Some(cause) = below {
            causes.push(cause.to_string());
            below = cause.source();
        }
        let problem = Problem { problem: self.code(), message: self.to_string(), causes, fields };
        let mut reply = Reply::new(status, Media::Json, serde_json::to_vec(&problem).unwrap_or_default());
        reply.challenge = matches!(self, Self::Anonymous);
        match self {
            Self::Method(error) => reply.allow.clone_from(&error.allowed),
            Self::Limited(limited) => reply.retry = Some(limited.retry),
            _ => {}
        }
        reply
    }
}
