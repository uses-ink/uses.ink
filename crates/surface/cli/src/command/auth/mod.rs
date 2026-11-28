//! `ink login`, `logout`, `whoami` and `ink key list|add|revoke`: a machine's key and the keys of
//! an account (decision 0068).
//!
//! The private key is made on this machine and is the secret: nothing is sent but its public half
//! and a fingerprint that both screens show, so a person compares them before approving. It is
//! kept in a file of its own ([`Keystore`]), never in the config file, an argument or the output.
//!
//! - `ink login --data <folder>` is the operator's way: it adds the new key's public half to the
//!   server's `operator-keys` file, which only whoever owns the data folder can write. No browser.
//! - `ink login` asks the server to enrol the key and waits, signing its polls with the key, while
//!   a person approves it. A key that is not approved is kept nowhere.
//! - `ink key add` makes a key for another machine, such as a CI job, the same way; its private
//!   key is printed once, to a file or a redirect and never to a terminal.

mod flags;
mod identity;
mod store;
mod view;
mod wire;

use std::io::IsTerminal as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Subcommand};
use clock::{Draw as _, System};
use control::signin::Ttl;
use edge_api::{OperatorFile, OperatorKey};
use hyper::Method;
use sig::{Fingerprint, Seed, Signer};

pub(crate) use flags::{Flags, Resolved};
pub(crate) use identity::Identity;
pub(crate) use store::{Keystore, StoreError};

use crate::deploy::{Endpoint, Failure, Needed, Retry, Wire};
use crate::grammar::tree::Heading;
use crate::output::error::Error;
use crate::output::exit::Exit;
use crate::output::term::Verbatim;
use crate::output::{Io, Say};
use crate::settings::{Environment, Preferences, Process};
use wire::{Enrolled, Items, Listed, Polled, Row, Whoami};

/// Which server a key command talks to.
#[derive(Debug, Args)]
#[command(next_help_heading = Heading::ACCOUNT)]
pub struct Server {
    /// The API's address, `http://host[:port]`; `account.api` in the config file.
    #[arg(long, value_name = "URL", env = "INK_API")]
    pub api: Option<String>,
    /// Seconds one call may take.
    #[arg(long, value_name = "SECONDS", env = "INK_TIMEOUT", default_value_t = 30)]
    pub timeout: u64,
}

/// `ink login`.
#[derive(Debug, Args)]
pub struct Login {
    /// What the person sees beside the fingerprint. [default: this machine's name]
    #[arg(long, value_name = "TEXT")]
    pub label: Option<String>,
    /// The server's data folder: add this key to its `operator-keys` file instead of asking a
    /// person to approve it. Whoever can write that folder is the operator.
    #[arg(long, value_name = "FOLDER")]
    pub data: Option<PathBuf>,
    /// Seconds to wait for a person to approve the key.
    #[arg(long, value_name = "SECONDS", default_value_t = 600)]
    pub wait: u64,
    /// Seconds between asks whether it was approved.
    #[arg(long, value_name = "SECONDS", default_value_t = 2)]
    pub poll: u64,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// `ink key <list|add|revoke>`: a group with no behaviour of its own.
#[derive(Debug, Args)]
pub struct Key {
    /// What to do with the keys.
    #[command(subcommand)]
    pub command: Keys,
}

/// What `ink key` does.
#[derive(Debug, Subcommand)]
pub enum Keys {
    /// The account's keys: kind, rights, end, the day each was last used.
    List(Server),
    /// Make a key for another machine, such as a CI job; a person approves it.
    Add(Add),
    /// End a key.
    Revoke(Revoke),
}

/// `ink key add`.
#[derive(Debug, Args)]
pub struct Add {
    /// A site's id: make a deploy key for that one site. Without it, a device key.
    #[arg(long, value_name = "ID")]
    pub site: Option<String>,
    /// What the person sees beside the fingerprint.
    #[arg(long, value_name = "TEXT", default_value = "ci")]
    pub label: String,
    /// The rights to give, such as `read,deploy`. [default: what the kind has by default]
    #[arg(long, value_name = "LIST")]
    pub right: Option<String>,
    /// How long the key lives, such as `90d`. [default: the server's, per kind]
    #[arg(long, value_name = "TTL")]
    pub expires: Option<Ttl>,
    /// Write the private key to this file (0600) instead of printing it.
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,
    /// Seconds to wait for a person to approve the key.
    #[arg(long, value_name = "SECONDS", default_value_t = 600)]
    pub wait: u64,
    /// Seconds between asks whether it was approved.
    #[arg(long, value_name = "SECONDS", default_value_t = 2)]
    pub poll: u64,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// `ink key revoke <FINGERPRINT>`.
#[derive(Debug, Args)]
pub struct Revoke {
    /// The key's fingerprint, as `ink key list` shows it.
    pub fingerprint: String,
    /// Which server.
    #[command(flatten)]
    pub server: Server,
}

/// A connection ready to make calls: the wire and the runtime they run on.
#[derive(Debug)]
pub(crate) struct Calls {
    pub(crate) wire: Wire,
    runtime: tokio::runtime::Runtime,
    pub(crate) api: String,
}

impl Server {
    /// The API address given by the flag, the variable or the config file.
    fn address(&self, preferences: &Preferences) -> Result<String, Failure> {
        self.api.clone().or_else(|| preferences.account.api.clone()).ok_or(Failure::Setting { needed: Needed::Api })
    }

    /// The calls, signed with `identity` or, for the ask nobody signs, with none.
    fn calls(
        &self,
        preferences: &Preferences,
        who: impl FnOnce(&str) -> Result<Option<Identity>, Failure>,
    ) -> Result<Calls, Failure> {
        let api = self.address(preferences)?;
        let endpoint = Endpoint::parse(&api)?;
        let identity = who(endpoint.authority())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| Failure::Runtime { source })?;
        let wire = Wire { endpoint, identity, budget: Duration::from_secs(self.timeout), retry: Retry::NONE };
        Ok(Calls { wire, runtime, api })
    }

    /// The calls signed as the key this machine holds.
    pub(crate) fn signed(&self, env: &impl Environment, preferences: &Preferences) -> Result<Calls, Failure> {
        self.calls(preferences, |authority| Identity::find(env, authority).map(Some))
    }
}

impl Calls {
    /// A JSON call.
    pub(crate) fn json<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
    ) -> Result<T, Failure> {
        let bytes = self.runtime.block_on(self.wire.call(method, path, body))?;
        serde_json::from_slice(&bytes).map_err(|source| Failure::Unreadable { source })
    }
}

/// Makes a key: the seed and what it is called to a person.
struct Fresh {
    seed: Seed,
}

impl Fresh {
    fn make() -> Result<Self, Failure> {
        let draw = || System.bytes().map_err(|source| Failure::Draw { source: Box::new(source) });
        Ok(Self { seed: Seed::from_draws(draw()?, draw()?) })
    }
}

/// The wait for a person: ask, show, poll with the new key's signature.
struct Enrolment<'a> {
    calls: &'a Calls,
    wait: Duration,
    poll: Duration,
}

impl Enrolment<'_> {
    /// Asks the server to enrol `key` with `ask`, shows what to approve, and waits. The key's
    /// end, once it was approved.
    fn run(
        &self,
        key: &Fresh,
        ask: &serde_json::Value,
        io: &mut Io<'_>,
    ) -> Result<(Enrolled, Option<jiff::Timestamp>), Error> {
        let public = Signer::from_seed(&key.seed).public();
        let mut body = serde_json::json!({ "key": public.to_string() });
        if let (Some(into), Some(from)) = (body.as_object_mut(), ask.as_object()) {
            into.extend(from.clone());
        }
        let body = serde_json::to_vec(&body).map_err(|source| Failure::Unreadable { source })?;
        let asked: Enrolled = self.calls.json(Method::POST, "/v1/enrol", body)?;
        let label = ask.get("label").and_then(|label| label.as_str()).unwrap_or_default();
        io.result(&view::Asking { ask: &asked, label, api: &self.calls.api })?;
        let identity = Identity::new(&key.seed, self.calls.wire.endpoint.authority())?;
        let polling = Wire {
            endpoint: self.calls.wire.endpoint.clone(),
            identity: Some(identity),
            budget: self.calls.wire.budget,
            retry: Retry::NONE,
        };
        let path = format!("/v1/enrol/{}", asked.id);
        let started = std::time::Instant::now();
        loop {
            let bytes = self.calls.runtime.block_on(polling.call(Method::GET, &path, Vec::new()))?;
            let polled: Polled = serde_json::from_slice(&bytes).map_err(|source| Failure::Unreadable { source })?;
            if polled.state == "approved" {
                return Ok((asked, polled.expires));
            }
            if started.elapsed() >= self.wait {
                return Err(Failure::Unapproved { after: self.wait }.into());
            }
            std::thread::sleep(self.poll);
        }
    }
}

impl Login {
    /// Enrols this machine's key.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let preferences = Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        self.enrol(&Process, &preferences, io)
    }

    fn label(&self) -> String {
        self.label
            .clone()
            .or_else(|| Process.get("HOSTNAME"))
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| "this machine".to_owned())
    }

    /// The operator's way: the key's public half is a line of the server's file.
    fn operator(data: &std::path::Path, key: &Fresh, label: &str) -> Result<(), Failure> {
        let line = format!("ed25519-v1 {} {label}", Signer::from_seed(&key.seed).public());
        let parsed = OperatorKey::parse(&line).map_err(|source| Failure::Operators { source })?;
        let file = OperatorFile::at(data.join("operator-keys"));
        for operator in &parsed {
            file.add(operator).map_err(|source| Failure::Operators { source })?;
        }
        Ok(())
    }

    fn enrol(&self, env: &impl Environment, preferences: &Preferences, io: &mut Io<'_>) -> Result<Exit, Error> {
        let store = Keystore::of(env).map_err(|source| Failure::Store { source })?;
        let calls = self.server.calls(preferences, |_| Ok(None))?;
        let authority = calls.wire.endpoint.authority().to_owned();
        if store.load(&authority).map_err(|source| Failure::Store { source })?.is_some() {
            return Err(Failure::Store { source: StoreError::Held { path: store.file(&authority) } }.into());
        }
        let key = Fresh::make()?;
        let label = self.label();
        let (fingerprint, expires, kind) = if let Some(data) = &self.data {
            Self::operator(data, &key, &label)?;
            (Signer::from_seed(&key.seed).id().fingerprint().to_string(), None, "operator")
        } else {
            let ask = serde_json::json!({ "label": label, "purpose": "device" });
            let wait =
                Enrolment { calls: &calls, wait: Duration::from_secs(self.wait), poll: Duration::from_secs(self.poll) };
            let (asked, expires) = wait.run(&key, &ask, io)?;
            (asked.fingerprint, expires, "device")
        };
        let kept = store.save(&authority, &key.seed).map_err(|source| Failure::Store { source })?.display().to_string();
        let joined = view::Joined { fingerprint: &fingerprint, label: &label, kind, expires, kept: Some(&kept) };
        io.result(&joined)?;
        let json = wire::Joined {
            schema: "uses.key/v1",
            fingerprint: &fingerprint,
            label: &label,
            kind,
            expires,
            kept: Some(kept.clone()),
            private_key: None,
        };
        io.json(&json)?;
        Ok(Exit::Done)
    }
}

impl Server {
    /// `ink logout`: the key revokes itself, then this machine forgets it.
    pub(crate) fn logout(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let preferences = Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        self.logout_with(&Process, &preferences, io)
    }

    fn logout_with(&self, env: &impl Environment, preferences: &Preferences, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.signed(env, preferences)?;
        let print =
            calls.wire.identity.as_ref().map_or_else(String::new, |identity| identity.id().fingerprint().to_string());
        let result: Result<Row, Failure> = calls.json(Method::DELETE, &format!("/v1/keys/{print}"), Vec::new());
        // a key the server already ended is as logged out as one it just ended; one it could not be asked about is not
        let gone = matches!(result, Ok(_) | Err(Failure::Refused { status: 401, .. }));
        if gone {
            let store = Keystore::of(env).map_err(|source| Failure::Store { source })?;
            store.remove(calls.wire.endpoint.authority()).map_err(|source| Failure::Store { source })?;
        }
        let row = result?;
        io.result(&view::Ended(&row))?;
        io.json(&wire::Ended { schema: "uses.key-ended/v1", key: &row })?;
        Ok(Exit::Done)
    }

    /// `ink whoami`.
    pub(crate) fn whoami(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let preferences = Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        let who: Whoami = self.signed(&Process, &preferences)?.json(Method::GET, "/v1/whoami", Vec::new())?;
        io.result(&view::Who(&who))?;
        io.json(&wire::Who { schema: "uses.whoami/v1", who: &who })?;
        Ok(Exit::Done)
    }
}

impl Keys {
    /// Runs the key command.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let preferences = Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        match self {
            Self::List(server) => {
                let items: Items<Row> =
                    server.signed(&Process, &preferences)?.json(Method::GET, "/v1/keys", Vec::new())?;
                io.result(&view::Keys(&items.items))?;
                io.json(&Listed { schema: "uses.keys/v1", keys: &items.items })?;
                Ok(Exit::Done)
            }
            Self::Revoke(revoke) => revoke.run(&preferences, io),
            Self::Add(add) => add.run(&preferences, io),
        }
    }
}

impl Revoke {
    fn run(self, preferences: &Preferences, io: &mut Io<'_>) -> Result<Exit, Error> {
        let print: Fingerprint =
            self.fingerprint.parse().map_err(|_| Failure::Unknown { written: self.fingerprint.clone() })?;
        let row: Row = self.server.signed(&Process, preferences)?.json(
            Method::DELETE,
            &format!("/v1/keys/{print}"),
            Vec::new(),
        )?;
        io.result(&view::Ended(&row))?;
        io.json(&wire::Ended { schema: "uses.key-ended/v1", key: &row })?;
        Ok(Exit::Done)
    }
}

impl Add {
    /// The ask as the server reads it.
    fn ask(&self) -> serde_json::Value {
        let kind = if self.site.is_some() { "deploy" } else { "device" };
        let mut ask = serde_json::json!({ "label": self.label, "purpose": kind });
        if let Some(object) = ask.as_object_mut() {
            if let Some(site) = &self.site {
                object.insert("site".into(), site.clone().into());
            }
            if let Some(right) = &self.right {
                object.insert("rights".into(), right.clone().into());
            }
            if let Some(expires) = self.expires {
                object.insert("lifetime".into(), expires.to_string().into());
            }
        }
        ask
    }

    /// Writes the private key to `--out`, mode 0600 and never over a file; where it was written.
    fn keep(&self, text: &sig::KeyText) -> Result<Option<String>, Failure> {
        let Some(path) = &self.out else { return Ok(None) };
        let failed = |source| Failure::Store { source: StoreError::Io { path: path.clone(), source } };
        let mut file =
            std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(failed)?;
        std::io::Write::write_all(&mut file, format!("{}\n", text.reveal()).as_bytes()).map_err(failed)?;
        Ok(Some(path.display().to_string()))
    }

    fn run(self, preferences: &Preferences, io: &mut Io<'_>) -> Result<Exit, Error> {
        // a private key is printed once, and never to a terminal: refuse before anyone approves it
        if self.out.is_none() && std::io::stdout().is_terminal() {
            return Err(Failure::PrintRefused.into());
        }
        let calls = self.server.calls(preferences, |_| Ok(None))?;
        let kind = if self.site.is_some() { "deploy" } else { "device" };
        let key = Fresh::make()?;
        let wait =
            Enrolment { calls: &calls, wait: Duration::from_secs(self.wait), poll: Duration::from_secs(self.poll) };
        let (asked, expires) = wait.run(&key, &self.ask(), io)?;
        let text = key.seed.text();
        let kept = self.keep(&text)?;
        let joined =
            view::Joined { fingerprint: &asked.fingerprint, label: &self.label, kind, expires, kept: kept.as_deref() };
        io.result(&joined)?;
        if kept.is_none() {
            let note =
                Verbatim("the private key follows on stdout, once; keep it as INK_KEY or INK_KEY_FILE".to_owned());
            io.aside(Say::Warnings, &note)?;
            io.line(text.reveal())?;
        }
        let json = wire::Joined {
            schema: "uses.key/v1",
            fingerprint: &asked.fingerprint,
            label: &self.label,
            kind,
            expires,
            kept: kept.clone(),
            private_key: kept.is_none().then(|| text.reveal()),
        };
        io.json(&json)?;
        Ok(Exit::Done)
    }
}
