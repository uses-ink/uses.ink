//! The API as the Worker runs it: what its variables say, and the one value that answers
//! requests over the ports a Worker has.

use std::cell::Cell;
use std::error::Error;
use std::fmt;

use authority_forge::Limits as Forges;
use clock::{Clock, Draw};
use control::keys::Settings;
use control::{AccountKind, Control, Id, NewAccount, Origin, Ports, Refusal};
use db::Db;
use edge_api::view::Problem;
use edge_api::{
    Answers, Api, Closing, Enrol, Hooks, Limits, Media, Nonces, Reply, Request, Routed, Signatures, Signing, Status,
    Window,
};
use edge_control::Settings as Lookup;
use jiff::Timestamp;
use sig::Audience;
use watch::Previews;

use crate::api::{Gate, Pinned};

/// Where a Worker's variables are read from.
pub trait Vars {
    /// The value of `name`, or none when it is not set.
    fn var(&self, name: &str) -> Option<String>;
}

/// Why the API could not be set up from the variables.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unready {
    /// A variable the Worker cannot run without is not set.
    Missing {
        /// Its name.
        name: &'static str,
    },
    /// A variable is set to something that is not usable.
    Invalid {
        /// Its name.
        name: &'static str,
        /// Why.
        source: Box<dyn Error + Send + Sync>,
    },
}

impl Unready {
    /// The error of a variable `name` that did not read, for `map_err`.
    fn invalid<E: Error + Send + Sync + 'static>(name: &'static str) -> impl FnOnce(E) -> Self {
        move |source| Self::Invalid { name, source: Box::new(source) }
    }
}

impl fmt::Display for Unready {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { name } => write!(out, "the variable {name} is not set"),
            Self::Invalid { name, .. } => write!(out, "the variable {name} is not usable"),
        }
    }
}

impl Error for Unready {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Missing { .. } => None,
            Self::Invalid { source, .. } => Some(source.as_ref()),
        }
    }
}

/// Reads variables as text and numbers, naming the one that failed.
struct Reading<'a, V>(&'a V);

impl<V: Vars> Reading<'_, V> {
    /// The text of `name`, which must be set.
    fn text(&self, name: &'static str) -> Result<String, Unready> {
        self.0.var(name).ok_or(Unready::Missing { name })
    }

    /// The number in `name`, or none when it is not set.
    fn number(&self, name: &'static str) -> Result<Option<u64>, Unready> {
        self.0.var(name).map(|text| text.trim().parse::<u64>().map_err(Unready::invalid(name))).transpose()
    }
}

/// What the Worker's variables say about its API.
#[derive(Debug)]
pub struct Hosting {
    origin: Origin,
    api: String,
    audience: Audience,
    operators: Pinned,
    limits: Limits,
    window: Window,
    lookup: Lookup,
}

impl Hosting {
    /// The account an operator's key signs in as: one fixed id, since a Worker has one operator.
    const OWNER: [u8; 16] = *b"ink-host-owner\0\0";

    /// The variables, by name: `ORIGIN` (where sites are served, such as `https://uses.ink`),
    /// `API_HOST` (the host that answers the API, with its port when it has one), `AUDIENCE`
    /// (what signatures are made for, `API_HOST` when it is not set), `OPERATOR_KEYS` (the text of
    /// the operator's keys file), and the limits `API_BODY`, `UPLOAD_BYTES`, `UPLOAD_FILES`,
    /// `UPLOAD_FILE`, `UPLOAD_TREE`, `UPLOAD_PART` and `EXPORT_BYTES`, and `AUTH_SKEW` (the seconds a signature may be off by).
    pub fn read(vars: &impl Vars) -> Result<Self, Unready> {
        let reading = Reading(vars);
        let url = model::ExternalUrl::parse(reading.text("ORIGIN")?).map_err(Unready::invalid("ORIGIN"))?;
        let origin = Origin::try_from(&url).map_err(Unready::invalid("ORIGIN"))?;
        let api = reading.text("API_HOST")?;
        let audience = Audience::parse(&vars.var("AUDIENCE").unwrap_or_else(|| api.clone()))
            .map_err(Unready::invalid("AUDIENCE"))?;
        let operators = Pinned::read(&vars.var("OPERATOR_KEYS").unwrap_or_default());
        let limits = Self::ceilings(&reading)?;
        let window = Self::skew(vars)?;
        let lookup = Self::lookup_of(&reading)?;
        Ok(Self { origin, api, audience, operators, limits, window, lookup })
    }

    /// The lookup of the serve path: `LOOKUP_TTL` seconds a live deploy is remembered (ten when it
    /// is not set; zero asks the control plane every time, so a suspension applies at once).
    fn lookup_of(reading: &Reading<'_, impl Vars>) -> Result<Lookup, Unready> {
        let mut lookup = Lookup::default();
        if let Some(seconds) = reading.number("LOOKUP_TTL")? {
            lookup.ttl = std::time::Duration::from_secs(seconds);
        }
        Ok(lookup)
    }

    /// How the serve path remembers what it looked up.
    pub const fn lookup(&self) -> Lookup {
        self.lookup
    }

    /// The seconds a signature may be off by, from `AUTH_SKEW`: the window of the signatures and of
    /// the nonce objects, which must be the same number.
    pub fn skew(vars: &impl Vars) -> Result<Window, Unready> {
        Reading(vars)
            .number("AUTH_SKEW")?
            .map_or(Ok(Window::DEFAULT), Window::new)
            .map_err(Unready::invalid("AUTH_SKEW"))
    }

    /// The limits of a Worker, which holds a request in memory: below the self-host's, and the
    /// operator's variables over them.
    fn ceilings(reading: &Reading<'_, impl Vars>) -> Result<Limits, Unready> {
        const MIB: u64 = 1024 * 1024;
        let mut limits =
            Limits { archive: 32 * MIB, file: 25 * MIB, part: 16 * MIB, export: 32 * MIB, ..Limits::default() };
        let fields = [
            ("API_BODY", &mut limits.body),
            ("UPLOAD_BYTES", &mut limits.archive),
            ("UPLOAD_FILES", &mut limits.files),
            ("UPLOAD_FILE", &mut limits.file),
            ("UPLOAD_TREE", &mut limits.tree),
            ("UPLOAD_PART", &mut limits.part),
            ("EXPORT_BYTES", &mut limits.export),
        ];
        for (name, field) in fields {
            if let Some(number) = reading.number(name)? {
                *field = number;
            }
        }
        Ok(limits)
    }

    /// The host that answers the API, as the request names it.
    pub fn api(&self) -> &str {
        &self.api
    }

    /// Where sites are served.
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }

    /// What an upload and a body may cost.
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// The operator's keys.
    pub const fn operators(&self) -> &Pinned {
        &self.operators
    }

    /// The API over `ports`, whose rows are `db`, remembering signatures in `nonces` (one store
    /// for the API's, one for the webhooks') and reading time and randomness from `clocks`.
    pub fn wire<P, N, K, D>(self, stores: (P, P::Db), nonces: (N, N), clocks: (K, D)) -> Wired<P, N, K, D>
    where
        P: Ports + Clone + 'static,
        P::Db: Db + Clone,
        N: Nonces,
        K: Clock + Clone,
        D: Draw + Clone,
    {
        let (ports, db) = stores;
        let owner: Id<AccountKind> = Id::from_bytes(Self::OWNER);
        let control = || Control::new(ports.clone(), self.origin.clone());
        let enrol = Enrol::new(db, Settings::default(), Closing::default());
        let mut signing = Signing::new(self.audience, owner);
        signing.window = self.window;
        // the nonce store outlives an isolate and keeps the barrier of the boot rule itself
        let signatures =
            Signatures::with_nonces(enrol.keys().clone(), (clocks.0.clone(), self.operators), signing, nonces.0)
                .booted(Timestamp::UNIX_EPOCH);
        let api = Api::new(control(), (signatures, enrol), clocks.clone(), self.limits);
        let hooks = Hooks::new(control(), nonces.1, clocks.clone(), (Forges::default(), Previews::DEFAULT));
        Wired {
            calls: Gate::new(Routed::new(api, hooks)),
            control: control(),
            owner,
            clock: clocks.0,
            ready: Cell::new(false),
            limits: self.limits,
            api: self.api,
        }
    }
}

/// The routes the API answers: signed calls and webhooks, behind the pending ones.
type Routes<P, N, K, D> = Gate<Routed<Api<P, Signatures<<P as Ports>::Db, K, Pinned, N>, K, D>, Hooks<P, N, K, D>>>;

/// The API a Worker answers with: signed calls, the webhooks, and the pending routes in front.
pub struct Wired<P: Ports, N, K, D> {
    calls: Routes<P, N, K, D>,
    control: Control<P>,
    owner: Id<AccountKind>,
    clock: K,
    ready: Cell<bool>,
    limits: Limits,
    api: String,
}

impl<P: Ports, N, K, D> fmt::Debug for Wired<P, N, K, D> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Wired").field("api", &self.api).field("ready", &self.ready.get()).finish_non_exhaustive()
    }
}

impl<P: Ports + 'static, N, K: Clock, D> Wired<P, N, K, D> {
    /// The host that answers the API.
    pub fn api(&self) -> &str {
        &self.api
    }

    /// What a body may cost, by route.
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// Makes the operator's account when it is not there. It is asked once for the life of the
    /// isolate, and again by the next request if it failed.
    async fn boot(&self) -> Result<(), control::Error<P>> {
        if self.ready.get() {
            return Ok(());
        }
        let account = NewAccount { id: self.owner, at: self.clock.now() };
        match self.control.add_account(account).await {
            Ok(_) => {}
            Err(error) if matches!(error.refusal(), Some(Refusal::Exists { .. })) => {}
            Err(error) => return Err(error),
        }
        self.ready.set(true);
        Ok(())
    }

    /// The answer when the control plane could not be made ready.
    fn unready(error: &control::Error<P>) -> Reply {
        let mut causes = Vec::new();
        let mut next = error.source();
        while let Some(cause) = next {
            causes.push(cause.to_string());
            next = cause.source();
        }
        let problem = Problem {
            problem: "control",
            message: "the control plane did not answer".to_owned(),
            causes,
            fields: std::collections::BTreeMap::new(),
        };
        Reply::new(Status::Internal, Media::Json, serde_json::to_vec(&problem).unwrap_or_default())
    }
}

impl<P, N, K, D> Answers for Wired<P, N, K, D>
where
    P: Ports + Clone + 'static,
    P::Db: Db,
    N: Nonces,
    K: Clock + Clone,
    D: Draw + Clone,
{
    async fn answer(&self, request: &Request) -> Reply {
        match self.boot().await {
            Ok(()) => self.calls.answer(request).await,
            Err(error) => Self::unready(&error),
        }
    }
}
