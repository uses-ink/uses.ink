//! Why a remote command did not do its work.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use control::JobState;
use model::DeployId;
use serde::Deserialize;

use crate::command::publish::LinkError;
use crate::output::exit::Exit;

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
pub struct ApiRefusal {
    /// A stable name for the refusal.
    pub problem: String,
    /// What is wrong, in a sentence.
    pub message: String,
    /// The data of the refusal, under the names of the diagnostic catalog.
    #[serde(default)]
    pub fields: std::collections::BTreeMap<String, String>,
}

impl ApiRefusal {
    /// What to do about it, for the refusals a person can fix.
    pub fn fix(&self) -> Option<&'static str> {
        Some(match self.problem.as_str() {
            "auth.signature.skewed" => "set this machine's clock, for example `timedatectl set-ntp true`",
            "auth.signature.refused" | "auth.key.unknown" => "run `ink login` to sign in again on this machine",
            "auth.key.revoked" | "auth.key.expired" => "run `ink login`; for CI, make a new key with `ink key add`",
            "auth.right.missing" => "use a key with that right, or approve one with `ink key add`",
            "auth.enrolment.expired" | "auth.enrolment.denied" | "auth.enrolment.limited" => "run `ink login` again",
            "domain.proof.missing" => {
                "publish the TXT record `ink domain add` showed, wait for your DNS to answer it, then run `ink domain prove` again"
            }
            "domain.lookup.failed" => {
                "the name was not refused, only unchecked: run `ink domain prove` again in a minute"
            }
            "domain.claim.unknown" => "run `ink domain add <NAME>` first; it prints the record to publish",
            "preview.label.unknown" => "run `ink preview list` for the labels that are served",
            "auth.server.restarting" => "wait a few seconds and run the command again",
            _ => return None,
        })
    }
}

impl fmt::Display for ApiRefusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}: {}", self.problem, self.message)
    }
}

impl std::error::Error for ApiRefusal {}

/// One line of a build's log that names a problem, as the server stored it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Logged(pub String);

impl fmt::Display for Logged {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl std::error::Error for Logged {}

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
    /// The text is no key fingerprint, so no key can be named by it.
    NoPrint {
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
        refusal: Option<ApiRefusal>,
    },
    /// The API's answer is not the JSON this reads.
    Unreadable {
        /// Why.
        source: serde_json::Error,
    },
    /// The private key could not be kept or read.
    Store {
        /// Why.
        source: crate::command::auth::StoreError,
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
    /// The folder is linked to no site, and nothing else says which.
    Unlinked {
        /// The folder.
        dir: PathBuf,
    },
    /// The link file cannot be used.
    Link {
        /// The file.
        path: PathBuf,
        /// Why.
        source: LinkError,
    },
    /// The folder is linked to another site already.
    Linked {
        /// The file.
        path: PathBuf,
        /// The site it names.
        site: String,
    },
    /// `ink init` was told no site and no handle.
    Nameless,
    /// The folder holds the data folder of an `ink serve`, which `ink push` would send whole.
    Holds {
        /// The first file of it, as the folder names it.
        path: String,
    },
    /// The tree of the folder is not a tree.
    Tree {
        /// Why.
        source: build::TreeError,
    },
    /// The server still lacked files after they were sent, round after round.
    Incomplete {
        /// How many files it lacked at the end.
        missing: usize,
        /// How many rounds were made.
        rounds: u32,
    },
    /// The upload stopped after `sent` files were stored; they stay, and the next push sends only
    /// what is left.
    Uploading {
        /// How many files the server took.
        sent: u64,
        /// What stopped it.
        source: Box<Failure>,
    },
    /// Something stopped a command that had queued a build: the build is the server's and keeps
    /// going. Says what the inner failure says; only what the command left behind differs.
    Following {
        /// The build, as the server named it.
        build: String,
        /// What stopped the command.
        source: Box<Failure>,
    },
    /// The build ended without a deploy.
    Ended {
        /// The build.
        build: String,
        /// How it ended.
        state: JobState,
        /// How many problems its log names.
        count: usize,
        /// The first of them.
        first: Option<Logged>,
    },
    /// The deploy is recorded and is not live: publishing is paused.
    Held {
        /// The deploy.
        deploy: DeployId,
    },
    /// The build did not end in the time given.
    Waited {
        /// The build.
        build: String,
        /// How long it was waited for.
        after: Duration,
    },
    /// The site does not list the build.
    Absent {
        /// The build, as written.
        build: String,
    },
    /// The export is not an archive of files this reads.
    Archive {
        /// Why.
        source: edge_api::tar::Error,
    },
    /// The folder to export into holds something.
    Occupied {
        /// The folder.
        path: PathBuf,
    },
    /// A folder or a file of the export could not be written.
    Placing {
        /// Where.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
}

impl Failure {
    /// The exit code: sign-in is 3, the network 4, a conflict 5, and what the person typed is 2.
    #[expect(clippy::too_many_lines, reason = "one arm for each way a call fails")]
    pub const fn exit(&self) -> Exit {
        match self {
            Self::Setting { needed: Needed::Key } | Self::KeyText { .. } | Self::Unapproved { .. } => Exit::Auth,
            Self::Setting { .. }
            | Self::Address { .. }
            | Self::Scheme { .. }
            | Self::Site { .. }
            | Self::Unknown { .. }
            | Self::NoPrint { .. }
            | Self::Ambiguous { .. }
            | Self::Request { .. }
            | Self::Store { .. }
            | Self::Audience { .. }
            | Self::Target { .. }
            | Self::Method { .. }
            | Self::Draw { .. }
            | Self::Operators { .. }
            | Self::Unlinked { .. }
            | Self::Link { .. }
            | Self::Linked { .. }
            | Self::Nameless
            | Self::Holds { .. }
            | Self::Tree { .. }
            | Self::Absent { .. }
            | Self::Occupied { .. }
            | Self::Placing { .. }
            | Self::PrintRefused => Exit::Usage,
            Self::Uploading { source, .. } | Self::Following { source, .. } => source.exit(),
            Self::Held { .. } => Exit::Conflict,
            Self::Ended { state: JobState::Refused, .. } => Exit::Content,
            Self::Ended { .. }
            | Self::Incomplete { .. }
            | Self::Archive { .. }
            | Self::Waited { .. }
            | Self::Runtime { .. }
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
            Self::NoPrint { .. } => "auth.key.unknown",
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
            Self::Unlinked { .. } => "link.file.missing",
            Self::Link { .. } => "link.file.unusable",
            Self::Linked { .. } => "link.file.exists",
            Self::Nameless => "init.target.missing",
            Self::Holds { .. } => "push.folder.holds-data",
            Self::Tree { .. } => "push.tree.invalid",
            Self::Incomplete { .. } => "push.upload.incomplete",
            Self::Uploading { source, .. } | Self::Following { source, .. } => source.kind(),
            Self::Ended { state: JobState::Refused, .. } => "push.build.refused",
            Self::Ended { .. } => "push.build.failed",
            Self::Held { .. } => "push.deploy.held",
            Self::Waited { .. } => "push.wait.expired",
            Self::Absent { .. } => "push.build.unknown",
            Self::Archive { .. } => "export.archive.invalid",
            Self::Occupied { .. } => "export.folder.occupied",
            Self::Placing { .. } => "export.folder.unwritable",
        }
    }

    /// Whether the same call may be sent again: the network failed, or the server was not there.
    pub(crate) fn transient(&self) -> bool {
        match self {
            Self::Connect { .. } | Self::Exchange { .. } | Self::Timeout { .. } => true,
            Self::Refused { status, .. } => matches!(*status, 502..=504),
            _ => false,
        }
    }

    /// How many files an upload stored before it stopped, when it stopped part way.
    pub(crate) fn stored(&self) -> Option<u64> {
        match self {
            Self::Uploading { sent, .. } => Some(*sent),
            _ => None,
        }
    }

    /// The build the server holds for this command, when it had queued one before it stopped.
    pub(crate) fn queued(&self) -> Option<&str> {
        match self {
            Self::Following { build, .. } => Some(build),
            _ => None,
        }
    }

    /// What to do about it: every way a call fails has one thing to say.
    #[expect(clippy::too_many_lines, reason = "one arm for each way a call fails")]
    pub(crate) fn remedy(&self) -> String {
        match self {
            Self::Setting { needed } => format!("give {}", needed.given()),
            Self::Address { .. } | Self::Scheme { .. } | Self::Audience { .. } => {
                "write the address as `http://host[:port]`, in `--api`, `INK_API` or the config file".to_owned()
            }
            Self::Site { .. } => "write the site's id as `ink init` prints it, 32 hexadecimal digits".to_owned(),
            Self::Unknown { .. } | Self::Ambiguous { .. } => {
                "write the start of a deploy id as `ink deploy list` shows it, enough of it to name one".to_owned()
            }
            Self::NoPrint { .. } => "write the key's fingerprint as `ink key list` shows it".to_owned(),
            Self::Runtime { .. }
            | Self::Request { .. }
            | Self::Target { .. }
            | Self::Method { .. }
            | Self::Tree { .. } => "this is a fault in ink: report it with the text above".to_owned(),
            Self::Draw { .. } => "check that the system gives random bytes, then run the command again".to_owned(),
            Self::Connect { .. } | Self::Exchange { .. } => {
                "check the address and that the server is running, then run the command again".to_owned()
            }
            Self::Timeout { .. } => "give a longer `--timeout`, or run the command again".to_owned(),
            Self::Refused { refusal: Some(refusal), .. } => refusal
                .fix()
                .map_or_else(|| "read the server's log for why, then run the command again".to_owned(), str::to_owned),
            Self::Refused { refusal: None, .. } => "read the server's log for why, then run the command again".to_owned(),
            Self::Unreadable { .. } => {
                "this ink and the server may be different versions: update the older one".to_owned()
            }
            Self::Store { source } => source.remedy().to_owned(),
            Self::KeyText { .. } => {
                "run `ink login` to make a key, or fix the text of `INK_KEY` or of the file `INK_KEY_FILE` names"
                    .to_owned()
            }
            Self::Operators { .. } => "give the data folder of the server you run, one you may write".to_owned(),
            Self::Unapproved { .. } => "run `ink login` again and approve the key before it ends".to_owned(),
            Self::PrintRefused => "send it to a file with `--out`, or redirect stdout".to_owned(),
            Self::Unlinked { .. } => "run `ink init --handle NAME` to link this folder, or give `--site ID`".to_owned(),
            Self::Link { .. } => "fix or remove the file, then run `ink init` again".to_owned(),
            Self::Linked { .. } => "run `ink init --force` to link it to another site".to_owned(),
            Self::Holds { .. } => {
                "keep the data folder of `ink serve` outside the site's folder, for example `--data ../state`".to_owned()
            }
            Self::Nameless => {
                "give `--handle NAME` to make a site, or `--site ID` to link one that exists".to_owned()
            }
            Self::Incomplete { .. } | Self::Waited { .. } | Self::Archive { .. } => {
                "run the command again; if it keeps failing, report it".to_owned()
            }
            Self::Uploading { source, .. } => {
                let said = source.remedy();
                format!("{said}; run `ink push` again to send what is left")
            }
            Self::Following { source, .. } => match source.as_ref() {
                Self::Refused { refusal: Some(refusal), .. } if refusal.problem == "auth.right.missing" => {
                    "this key may queue builds and not read them: push with `--no-wait`, or make a key with `ink key add --right read,deploy`".to_owned()
                }
                other => other.remedy(),
            },
            Self::Ended { state: JobState::Refused, .. } => "fix what the build said, then push again".to_owned(),
            Self::Ended { .. } => "run `ink push` again; if it keeps failing, report it".to_owned(),
            Self::Held { .. } => "run `ink deploy resume` to deploy this and new pushes".to_owned(),
            Self::Absent { .. } => "`ink status` lists the builds the site holds".to_owned(),
            Self::Occupied { .. } => "give an empty or new folder".to_owned(),
            Self::Placing { .. } => {
                "check that the folder may be written and has room, then run the command again".to_owned()
            }
        }
    }
}

impl fmt::Display for Failure {
    #[expect(clippy::too_many_lines, reason = "one arm of prose for each way a call fails")]
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Setting { needed } => write!(out, "no {} is set", needed.noun()),
            Self::Address { written, .. } => write!(out, "`{written}` is not an address"),
            Self::Scheme { written } => {
                write!(out, "`{written}` is not `http://host[:port]`: this ink speaks plain http only")
            }
            Self::Site { written, .. } => write!(out, "`{written}` is not a site id"),
            Self::Unknown { written } => write!(out, "no deploy of this site starts with `{written}`"),
            Self::NoPrint { written } => write!(out, "`{written}` is not the fingerprint of a key"),
            Self::Ambiguous { written, count } => {
                write!(out, "{count} deploys start with `{written}`: write more of it")
            }
            Self::Runtime { .. } => write!(out, "the runtime for the call could not start"),
            Self::Connect { address, .. } => write!(out, "the API at {address} could not be reached"),
            Self::Exchange { .. } => write!(out, "the exchange with the API broke off"),
            Self::Request { .. } => write!(out, "the request could not be built"),
            Self::Timeout { after } => write!(out, "the API did not answer in {} seconds", after.as_secs()),
            Self::Refused { status, .. } => write!(out, "the API refused the request ({status})"),
            Self::Unreadable { .. } => write!(out, "the API's answer is not what this ink reads"),
            Self::Store { .. } => write!(out, "the private key could not be kept or read"),
            Self::KeyText { from, .. } => write!(out, "{from} does not hold a key"),
            Self::Audience { written, .. } => {
                write!(out, "`{written}` is not an authority a request can be signed for")
            }
            Self::Target { .. } => write!(out, "the request target is not canonical, so it was not signed"),
            Self::Method { .. } => write!(out, "the method is not one a request is signed with"),
            Self::Draw { .. } => write!(out, "no random bytes were available"),
            Self::Operators { .. } => write!(out, "the operator keys of the data folder could not be read or written"),
            Self::Unapproved { after } => write!(out, "nobody approved the key in {} seconds", after.as_secs()),
            Self::PrintRefused => write!(out, "the private key would be printed to a terminal"),
            Self::Unlinked { dir } => write!(out, "{} is linked to no site: it has no .uses/site", dir.display()),
            Self::Link { path, .. } => write!(out, "the link file {} cannot be used", path.display()),
            Self::Linked { path, site } => {
                write!(out, "{} links this folder to the site {site} already", path.display())
            }
            Self::Nameless => write!(out, "no site is named: `ink init` links a folder to a site or makes one"),
            Self::Holds { path } => write!(
                out,
                "the folder holds {path}, which belongs to the data folder of an ink server: it holds the database and the signing key, and a push would publish them"
            ),
            Self::Tree { .. } => write!(out, "the folder's files do not make a tree"),
            Self::Incomplete { missing, rounds } => {
                write!(out, "the server still lacked {missing} files after {rounds} rounds of sending them")
            }
            Self::Uploading { sent, .. } => write!(out, "the upload stopped after the server took {sent} files"),
            Self::Following { source, .. } => fmt::Display::fmt(source, out),
            Self::Ended { build, state, count, .. } => match state {
                JobState::Refused => write!(out, "the build {build} was refused: {count} problems stop the site"),
                _ => write!(out, "the build {build} {state} without making a deploy"),
            },
            Self::Held { deploy } => {
                write!(out, "the deploy {deploy} is recorded and is not live: publishing is paused after a rollback")
            }
            Self::Waited { build, after } => {
                write!(out, "the build {build} did not end in {} seconds; it is still going", after.as_secs())
            }
            Self::Absent { build } => write!(out, "the site lists no build `{build}`"),
            Self::Archive { .. } => write!(out, "the export is not an archive of files"),
            Self::Occupied { path } => write!(out, "{} is not empty", path.display()),
            Self::Placing { path, .. } => write!(out, "{} could not be written", path.display()),
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
            Self::Runtime { source } | Self::Connect { source, .. } | Self::Placing { source, .. } => Some(source),
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
            Self::Link { source, .. } => Some(source),
            Self::Tree { source } => Some(source),
            Self::Uploading { source, .. } => Some(source.as_ref()),
            Self::Following { source, .. } => source.source(),
            Self::Archive { source } => Some(source),
            Self::Ended { first: Some(first), .. } => Some(first),
            Self::Refused { refusal, .. } => refusal.as_ref().map(|cause| cause as &(dyn std::error::Error + 'static)),
            Self::Setting { .. }
            | Self::Scheme { .. }
            | Self::Unknown { .. }
            | Self::NoPrint { .. }
            | Self::Ambiguous { .. }
            | Self::Timeout { .. }
            | Self::Unapproved { .. }
            | Self::Unlinked { .. }
            | Self::Linked { .. }
            | Self::Nameless
            | Self::Holds { .. }
            | Self::Incomplete { .. }
            | Self::Ended { first: None, .. }
            | Self::Held { .. }
            | Self::Waited { .. }
            | Self::Absent { .. }
            | Self::Occupied { .. }
            | Self::PrintRefused => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiRefusal, Failure};
    use std::collections::BTreeMap;

    #[test]
    fn a_refusal_says_its_message_once_and_its_name_as_the_cause() {
        let refusal = ApiRefusal {
            problem: "domain.claim.unknown".to_owned(),
            message: "there is no claim".to_owned(),
            fields: BTreeMap::new(),
        };
        let failure = Failure::Refused { status: 404, refusal: Some(refusal) };
        let head = failure.to_string();
        let cause = std::error::Error::source(&failure).map_or_else(String::new, ToString::to_string);
        assert_eq!(head, "the API refused the request (404)");
        assert_eq!(cause, "domain.claim.unknown: there is no claim");
    }

    #[test]
    fn a_key_that_is_no_fingerprint_is_a_key_not_a_deploy() {
        let failure = Failure::NoPrint { written: "nosuch".to_owned() };
        assert_eq!(failure.kind(), "auth.key.unknown");
        assert!(failure.remedy().contains("ink key list"));
        assert_eq!(failure.to_string(), "`nosuch` is not the fingerprint of a key");
    }
}
