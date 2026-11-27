//! Why a remote command did not do its work.

use std::fmt;
use std::time::Duration;

use serde::Deserialize;

use crate::exit::Exit;

/// A setting a remote command cannot do without.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Needed {
    /// The address of the API.
    Api,
    /// The site.
    Site,
    /// The key that signs requests.
    Key,
}

impl Needed {
    /// How the setting is given, flag first.
    const fn given(self) -> &'static str {
        match self {
            Self::Api => "`--api`, `INK_API` or `api` under `[account]` in the config file",
            Self::Site => "`--site`, `INK_SITE` or `site` under `[account]` in the config file",
            Self::Key => "`ink login`, or `INK_KEY` or `INK_KEY_FILE` for a CI job",
        }
    }
}

/// An error answer of the API: `{problem, message, causes, fields}`.
#[derive(Clone, Debug, Deserialize)]
pub struct Said {
    /// A stable name for the refusal.
    pub problem: String,
    /// What is wrong, in a sentence.
    pub message: String,
    /// The data of the refusal, under the names of the diagnostic catalog.
    #[serde(default)]
    pub fields: std::collections::BTreeMap<String, String>,
}

impl Said {
    /// What to do about it, for the refusals a person can fix.
    pub fn fix(&self) -> Option<&'static str> {
        Some(match self.problem.as_str() {
            "auth.signature.skewed" => "set this machine's clock, for example `timedatectl set-ntp true`",
            "auth.signature.refused" | "auth.key.unknown" => "run `ink login` to sign in again on this machine",
            "auth.key.revoked" | "auth.key.expired" => "run `ink login`; for CI, make a new key with `ink key add`",
            "auth.right.missing" => "use a key with that right, or approve one with `ink key add`",
            "auth.enrolment.expired" | "auth.enrolment.denied" | "auth.enrolment.limited" => "run `ink login` again",
            "auth.server.restarting" => "wait a few seconds and run the command again",
            _ => return None,
        })
    }
}

impl fmt::Display for Said {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}: {}", self.problem, self.message)
    }
}

impl std::error::Error for Said {}

/// A remote command that stopped.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failure {
    /// A setting is missing.
    Setting {
        /// Which.
        needed: Needed,
    },
    /// The address of the API is no address.
    Address {
        /// As written.
        written: String,
        /// Why not.
        source: hyper::http::uri::InvalidUri,
    },
    /// The address is not `http://host[:port]`: this ink holds no TLS.
    Scheme {
        /// As written.
        written: String,
    },
    /// The site is no id.
    Site {
        /// As written.
        written: String,
        /// Why not.
        source: control::IdError,
    },
    /// No deploy of the history starts with these characters.
    Unknown {
        /// As written.
        written: String,
    },
    /// Several deploys of the history start with these characters.
    Ambiguous {
        /// As written.
        written: String,
        /// How many.
        count: usize,
    },
    /// The runtime of the call could not start.
    Runtime {
        /// What the system said.
        source: std::io::Error,
    },
    /// The API could not be connected to.
    Connect {
        /// The address, host and port.
        address: String,
        /// What the system said.
        source: std::io::Error,
    },
    /// The exchange broke off.
    Exchange {
        /// Why.
        source: hyper::Error,
    },
    /// The request could not be built, such as for a token with a character a header cannot hold.
    Request {
        /// Why.
        source: hyper::http::Error,
    },
    /// The API did not answer in time.
    Timeout {
        /// How long it was given.
        after: Duration,
    },
    /// The API refused.
    Refused {
        /// The HTTP status.
        status: u16,
        /// What it said, when it said it in the shape it uses.
        said: Option<Said>,
    },
    /// The API's answer is not the JSON this reads.
    Unreadable {
        /// Why.
        source: serde_json::Error,
    },
    /// The private key could not be kept or read.
    Store {
        /// Why.
        source: crate::auth::StoreError,
    },
    /// The key in `INK_KEY` or a key file is not a key.
    KeyText {
        /// Where it came from.
        from: String,
        /// Why not.
        source: sig::KeyError,
    },
    /// The API's authority is no audience of a signature.
    Audience {
        /// The authority.
        written: String,
        /// Why not.
        source: sig::AudienceError,
    },
    /// The request target is not canonical, so it cannot be signed.
    Target {
        /// Why not.
        source: sig::NotCanonical,
    },
    /// The method is not one a signature names.
    Method {
        /// Why not.
        source: sig::MethodError,
    },
    /// No random bytes were available for a key or a nonce.
    Draw {
        /// Why.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// The operator's keys file could not be read or written.
    Operators {
        /// Why.
        source: edge_api::OperatorError,
    },
    /// Nobody approved the key before the wait ended.
    Unapproved {
        /// How long it waited.
        after: Duration,
    },
    /// The private key would be printed to a terminal.
    PrintRefused,
}

impl Failure {
    /// The exit code: sign-in is 3, the network 4, a conflict 5, and what the person typed is 2.
    pub const fn exit(&self) -> Exit {
        match self {
            Self::Setting { needed: Needed::Key } | Self::KeyText { .. } | Self::Unapproved { .. } => Exit::Auth,
            Self::Setting { .. }
            | Self::Address { .. }
            | Self::Scheme { .. }
            | Self::Site { .. }
            | Self::Unknown { .. }
            | Self::Ambiguous { .. }
            | Self::Request { .. }
            | Self::Store { .. }
            | Self::Audience { .. }
            | Self::Target { .. }
            | Self::Method { .. }
            | Self::Draw { .. }
            | Self::Operators { .. }
            | Self::PrintRefused => Exit::Usage,
            Self::Runtime { .. }
            | Self::Connect { .. }
            | Self::Exchange { .. }
            | Self::Timeout { .. }
            | Self::Unreadable { .. } => Exit::Network,
            Self::Refused { status, .. } => match *status {
                401 | 403 => Exit::Auth,
                409 => Exit::Conflict,
                500..=599 => Exit::Network,
                _ => Exit::Usage,
            },
        }
    }

    /// The name `--json` gives it.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Setting { .. } => "deploy.setting.missing",
            Self::Address { .. } | Self::Scheme { .. } => "deploy.api.invalid",
            Self::Site { .. } => "deploy.site.invalid",
            Self::Unknown { .. } => "deploy.id.unknown",
            Self::Ambiguous { .. } => "deploy.id.ambiguous",
            Self::Runtime { .. } => "deploy.runtime.failed",
            Self::Connect { .. } => "deploy.api.unreachable",
            Self::Exchange { .. } | Self::Request { .. } => "deploy.api.broken",
            Self::Timeout { .. } => "deploy.api.slow",
            Self::Refused { .. } => "deploy.api.refused",
            Self::Unreadable { .. } => "deploy.api.unreadable",
            Self::Store { .. } => "auth.store.failed",
            Self::KeyText { .. } => "auth.key.invalid",
            Self::Audience { .. } => "auth.audience.invalid",
            Self::Target { .. } | Self::Method { .. } => "auth.request.unsignable",
            Self::Draw { .. } => "auth.random.failed",
            Self::Operators { .. } => "auth.operator.unreadable",
            Self::Unapproved { .. } => "auth.enrolment.expired",
            Self::PrintRefused => "auth.key.print-refused",
        }
    }
}

impl fmt::Display for Failure {
    #[expect(clippy::too_many_lines, reason = "one arm of prose for each way a call fails")]
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Setting { needed } => write!(out, "no {} is set: give {}", needed.noun(), needed.given()),
            Self::Address { written, .. } => write!(out, "`{written}` is not an address"),
            Self::Scheme { written } => {
                write!(out, "`{written}` is not `http://host[:port]`: this ink speaks plain http only")
            }
            Self::Site { written, .. } => write!(out, "`{written}` is not a site id"),
            Self::Unknown { written } => write!(out, "no deploy of this site starts with `{written}`"),
            Self::Ambiguous { written, count } => {
                write!(out, "{count} deploys start with `{written}`: write more of it")
            }
            Self::Runtime { .. } => write!(out, "the runtime for the call could not start"),
            Self::Connect { address, .. } => write!(out, "the API at {address} could not be reached"),
            Self::Exchange { .. } => write!(out, "the exchange with the API broke off"),
            Self::Request { .. } => write!(out, "the request could not be built"),
            Self::Timeout { after } => write!(out, "the API did not answer in {} seconds", after.as_secs()),
            Self::Refused { status, said: Some(said) } => match said.fix() {
                Some(fix) => write!(out, "the API refused the request ({status}): {}; {fix}", said.message),
                None => write!(out, "the API refused the request ({status}): {}", said.message),
            },
            Self::Refused { status, said: None } => write!(out, "the API refused the request ({status})"),
            Self::Unreadable { .. } => write!(out, "the API's answer is not what this ink reads"),
            Self::Store { .. } => write!(out, "the private key could not be kept or read"),
            Self::KeyText { from, .. } => write!(out, "{from} does not hold a key: run `ink login` to make one"),
            Self::Audience { written, .. } => {
                write!(out, "`{written}` is not an authority a request can be signed for")
            }
            Self::Target { .. } => write!(out, "the request target is not canonical, so it was not signed"),
            Self::Method { .. } => write!(out, "the method is not one a request is signed with"),
            Self::Draw { .. } => write!(out, "no random bytes were available"),
            Self::Operators { .. } => write!(out, "the operator keys of the data folder could not be read or written"),
            Self::Unapproved { after } => {
                write!(out, "nobody approved the key in {} seconds: run `ink login` again", after.as_secs())
            }
            Self::PrintRefused => {
                write!(
                    out,
                    "the private key would be printed to a terminal: send it to a file with `--out`, or redirect stdout"
                )
            }
        }
    }
}

impl Needed {
    /// What it is called.
    const fn noun(self) -> &'static str {
        match self {
            Self::Api => "API address",
            Self::Site => "site",
            Self::Key => "key",
        }
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Address { source, .. } => Some(source),
            Self::Site { source, .. } => Some(source),
            Self::Runtime { source } | Self::Connect { source, .. } => Some(source),
            Self::Exchange { source } => Some(source),
            Self::Request { source } => Some(source),
            Self::Unreadable { source } => Some(source),
            Self::Store { source } => Some(source),
            Self::KeyText { source, .. } => Some(source),
            Self::Audience { source, .. } => Some(source),
            Self::Target { source } => Some(source),
            Self::Method { source } => Some(source),
            Self::Draw { source } => Some(source.as_ref()),
            Self::Operators { source } => Some(source),
            Self::Refused { said, .. } => said.as_ref().map(|said| said as &(dyn std::error::Error + 'static)),
            Self::Setting { .. }
            | Self::Scheme { .. }
            | Self::Unknown { .. }
            | Self::Ambiguous { .. }
            | Self::Timeout { .. }
            | Self::Unapproved { .. }
            | Self::PrintRefused => None,
        }
    }
}
