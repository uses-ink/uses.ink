//! Passkeys over the API crate's JSON (decision 0068, `docs/auth.md` section 4): the routes under
//! `/v1/passkeys`, which the dashboard's script calls with `fetch`.
//!
//! | Route | Needs | What it does |
//! |---|---|---|
//! | `POST /v1/passkeys/options` `{"label"}` | session | Starts a registration: the options for `navigator.credentials.create()`, and the ceremony cookie. |
//! | `POST /v1/passkeys` `{"label","credential"}` | session, ceremony | Verifies the registration and stores the passkey. `201`. |
//! | `POST /v1/passkeys/challenge` | nothing | Starts a sign-in with no username: the options for `navigator.credentials.get()`, and the ceremony cookie. |
//! | `POST /v1/passkeys/session` `{credential}` | ceremony | Verifies the assertion and begins a session: the session cookie, and no secret in the body. |
//! | `GET /v1/passkeys` | session | The account's passkeys. |
//! | `DELETE /v1/passkeys/{id}` | session | Removes one, and ends the sessions it made. |
//!
//! - **Cookies, never signatures.** These routes read the session and ceremony cookies and
//!   ignore an `Authorization` header; the [`crate::Api`] reads signatures and ignores cookies. A
//!   session made by a passkey is a cookie only: its secret is never in a body, so it cannot be
//!   made to sign anything.
//! - **Origin.** Every route that changes state needs an `Origin` of the relying party's list, or
//!   `Sec-Fetch-Site: same-origin` when there is none (the Fetch Metadata defence); `SameSite=Lax`
//!   is the first line. The WebAuthn client data carries the origin as well, checked by the verifier.
//! - **A ceremony is bound to the browser** by `__Host-ink-ceremony`, whose secret's digest is the
//!   row. It is spent before the reply is verified, so a reply is tried once, right or wrong.
//! - **The relying party id** is the first thing every route checks against what the first passkey
//!   was made under: a changed domain is `auth.relying-party.changed`, not a silent failure.

mod route;
mod stop;

use std::time::Duration;

use clock::{Clock, Draw};
use control::AccountKind;
use control::Id;
use control::keys::Label;
use control::passkeys::{
    Asserted, Audit, Authenticating, Begin, Created, Flow, Fresh, Passkey, Passkeys, Person, Reason, Refusal,
    Registering, Registry, Relying,
};
use control::recovery::{Code, Codes};
use control::signin::{Entered, Passed, Proof, Session, SignIn};
use db::Db;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

pub use route::Step;
pub use stop::Stop;

use crate::auth::cookie::Jar;
use crate::auth::signin::{Draws, Quota, Rate};
use crate::body::Unreadable;
use crate::reply::{Media, Payload, Reply, Request, Status};
use crate::{Answers, Owner};

/// How the passkey routes are set up. Every field is the operator's.
#[derive(Debug)]
pub struct Passing {
    /// The session cookie: the same jar the sign-in routes use.
    pub session: Jar,
    /// The ceremony cookie.
    pub ceremony: Jar,
    /// Who passkeys are made for, and which origins may ask.
    pub relying: Relying,
}

/// The passkey routes over the stores, a clock, a source of randomness and a verifier.
#[derive(Debug)]
pub struct Ceremonies<D, K, R, P> {
    entry: SignIn<D>,
    registry: Registry<D>,
    clock: K,
    draw: R,
    passkeys: P,
    config: Passing,
    challenge: Quota,
    codes: Codes<D>,
    recovery: Quota,
}

/// The body of `POST /v1/passkeys/options`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Naming {
    label: String,
}

/// The body of `POST /v1/passkeys`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registered {
    label: String,
    credential: Created,
}

/// The body of `POST /v1/passkeys/session`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asking {
    credential: Asserted,
}

/// A passkey as listed. The public key and the counter stay in the row.
#[derive(Serialize)]
struct Listed {
    id: String,
    label: String,
    created: Timestamp,
    used: Option<String>,
    synced: bool,
    backed_up: bool,
    transports: Vec<&'static str>,
}

impl From<&Passkey> for Listed {
    fn from(passkey: &Passkey) -> Self {
        Self {
            id: passkey.id.to_string(),
            label: passkey.label.to_string(),
            created: passkey.created,
            used: passkey.used.map(|day| day.to_string()),
            synced: passkey.stored.eligible,
            backed_up: passkey.stored.backed_up,
            transports: passkey.stored.transports.iter().map(|transport| transport.name()).collect(),
        }
    }
}

/// A passkey just registered.
#[derive(Serialize)]
struct Added {
    id: String,
    label: String,
}

/// A session just begun: when it ends, and no secret.
#[derive(Serialize)]
struct Began {
    account: Id<AccountKind>,
    id: Id<control::SessionKind>,
    expires: Timestamp,
    limit: Timestamp,
}

/// The body of `POST /v1/recovery`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recovering {
    code: String,
    label: String,
}

/// How many recovery codes are unspent.
#[derive(Serialize)]
struct Left {
    remaining: u64,
}

/// A new set of recovery codes, shown once.
#[derive(Serialize)]
struct Minted {
    codes: Vec<String>,
}

/// A passkey removed.
#[derive(Serialize)]
struct Removed {
    removed: bool,
}

impl<D: Db + Clone, K: Clock, R: Draw, P: Passkeys> Ceremonies<D, K, R, P> {
    /// How many sources are remembered at once.
    const TRACKED: usize = 10_000;

    /// The passkey routes over `stores` (sign-in and passkeys), with the time from `clocks.0` and
    /// randomness from `clocks.1`, verifying with `passkeys`.
    pub fn new(stores: (SignIn<D>, Registry<D>), clocks: (K, R), passkeys: P, config: Passing) -> Self {
        let challenge = Quota::new(Quota::CHALLENGES, Self::TRACKED);
        let (codes, recovery) = (stores.1.codes(), Quota::new(Quota::RECOVERIES, Self::TRACKED));
        Self {
            entry: stores.0,
            registry: stores.1,
            clock: clocks.0,
            draw: clocks.1,
            passkeys,
            config,
            challenge,
            codes,
            recovery,
        }
    }

    /// The same routes with a different limit on spending recovery codes, per source. The default
    /// is [`Quota::RECOVERIES`].
    #[must_use]
    pub fn with_recoveries(mut self, rate: Rate) -> Self {
        self.recovery = Quota::new(rate, Self::TRACKED);
        self
    }

    /// The same routes with a different limit on starting a sign-in, per source. The default is
    /// [`Quota::CHALLENGES`].
    #[must_use]
    pub fn with_challenges(mut self, rate: Rate) -> Self {
        self.challenge = Quota::new(rate, Self::TRACKED);
        self
    }

    /// How it is set up.
    pub const fn config(&self) -> &Passing {
        &self.config
    }

    /// Answers `request`. Never fails: a failure is a reply that says what went wrong.
    pub async fn respond(&self, request: &Request) -> Reply {
        match self.answer(request).await {
            Ok(reply) => reply,
            Err(stop) => {
                let mut reply = stop.reply();
                if matches!(stop, Stop::Anonymous) && request.cookie.is_some() {
                    reply.cookie = Some(self.config.session.clear());
                }
                reply
            }
        }
    }

    async fn answer(&self, request: &Request) -> Result<Reply, Stop> {
        let step = Step::parse(request.method, &request.path).ok_or(Stop::Route)?.map_err(Stop::Method)?;
        let body = match &request.body {
            Payload::Whole(body) => body.as_slice(),
            Payload::Over { limit } => return Err(Stop::Large { limit: *limit }),
        };
        if step.mutates() {
            self.origin(request)?;
        }
        self.registry.check(self.config.relying.id()).await.map_err(Stop::stored)?;
        let now = self.clock.now();
        match step {
            Step::Options => self.options(request, body, now).await,
            Step::Register => self.register(request, body, now).await,
            Step::Challenge => self.challenge(request, now).await,
            Step::Assert => self.assert(request, body, now).await,
            Step::List => self.list(request, now).await,
            Step::Remove(id) => self.remove(request, id, now).await,
            Step::Left => self.left(request, now).await,
            Step::Mint => self.mint(request, now).await,
            Step::Recover => self.recover(body, request, now).await,
        }
    }

    /// The Fetch Metadata defence of the OWASP CSRF cheat sheet: `Origin` must be one of the
    /// configured origins exactly; with no `Origin`, the browser must say `same-origin`.
    fn origin(&self, request: &Request) -> Result<(), Stop> {
        match (&request.origin, &request.site) {
            (Some(origin), _) if self.config.relying.allows(origin) => Ok(()),
            (None, Some(site)) if site == "same-origin" => Ok(()),
            _ => Err(Stop::Origin),
        }
    }

    fn json<T: Serialize>(status: Status, value: &T) -> Result<Reply, Stop> {
        let body = serde_json::to_vec(value).map_err(Stop::Json)?;
        Ok(Reply::new(status, Media::Json, body))
    }

    fn drawn(&self) -> Result<control::signin::Drawn, Stop> {
        Draws(&self.draw).drawn().map_err(|error| Stop::Draw(Box::new(error)))
    }

    /// The session of the request's cookie, which is the only credential these routes read.
    async fn session(&self, request: &Request, now: Timestamp) -> Result<Session, Stop> {
        let proof = request.cookie.as_deref().and_then(|header| self.config.session.read(header));
        let session = match proof {
            Some(proof) => self.entry.resolve(&proof, now).await.map_err(Stop::signed)?,
            None => None,
        };
        session.ok_or(Stop::Anonymous)
    }

    /// The secret of the ceremony cookie, if the request has one.
    fn held(&self, request: &Request) -> Option<Proof> {
        request.cookie.as_deref().and_then(|header| self.config.ceremony.read(header))
    }

    /// Keeps a ceremony, and puts its secret in the reply's ceremony cookie.
    async fn begin(&self, begin: Begin<'_>, now: Timestamp, reply: &mut Reply) -> Result<(), Stop> {
        self.registry.begin(begin, now).await.map_err(Stop::stored)?;
        reply.ceremony = Some(self.config.ceremony.set(begin.proof, Registry::<D>::LIFE));
        Ok(())
    }

    /// Spends the ceremony of the request's cookie.
    async fn spent(&self, request: &Request, flow: Flow, now: Timestamp) -> Result<control::passkeys::Spent, Stop> {
        let proof = self.held(request).ok_or(Stop::Challenge)?;
        self.registry.spend(&proof, flow, now).await.map_err(Stop::stored)?.ok_or(Stop::Challenge)
    }

    /// Refuses a session that may not change credentials now: one no passkey signed in within the
    /// step-up window, on an account that has a passkey.
    async fn fresh(&self, session: &Session, now: Timestamp) -> Result<(), Stop> {
        if self.entry.stepped(session, now).await.map_err(Stop::signed)? {
            Ok(())
        } else {
            Err(Stop::Step { window: self.entry.step_up() })
        }
    }

    async fn options(&self, request: &Request, body: &[u8], now: Timestamp) -> Result<Reply, Stop> {
        let session = self.session(request, now).await?;
        self.fresh(&session, now).await?;
        let naming: Naming = Unreadable::read(body).map_err(Stop::Body)?;
        let label = Label::parse(&naming.label).map_err(Stop::Label)?;
        self.starting(session.account, label, now).await
    }

    /// Starts registering a passkey for `account`: the options for `navigator.credentials.create()`
    /// and the ceremony cookie that binds the reply to this browser.
    async fn starting(&self, account: Id<AccountKind>, label: Label, now: Timestamp) -> Result<Reply, Stop> {
        let existing: Vec<_> = self
            .registry
            .list(account)
            .await
            .map_err(Stop::stored)?
            .into_iter()
            .map(|passkey| passkey.stored.credential)
            .collect();
        let who = Person { account, label };
        let (creation, Registering(state)) =
            self.passkeys.start_registration(&who, &existing).map_err(|error| Stop::Verifier(Box::new(error)))?;
        let drawn = self.drawn()?;
        let mut reply = Self::json(Status::Ok, &creation)?;
        let begin = Begin { flow: Flow::Register, account: Some(account), state: &state, proof: &drawn.proof };
        self.begin(begin, now, &mut reply).await?;
        Ok(reply)
    }

    /// How many recovery codes are unspent.
    async fn left(&self, request: &Request, now: Timestamp) -> Result<Reply, Stop> {
        let session = self.session(request, now).await?;
        let remaining = self.codes.left(session.account).await.map_err(Stop::stored_codes)?;
        Self::json(Status::Ok, &Left { remaining })
    }

    /// Makes a new set of recovery codes and shows them, once. The old set ends in the same batch.
    async fn mint(&self, request: &Request, now: Timestamp) -> Result<Reply, Stop> {
        let session = self.session(request, now).await?;
        self.fresh(&session, now).await?;
        let drawn = Draws(&self.draw).minted().map_err(|error| Stop::Draw(Box::new(error)))?;
        let codes = self.codes.mint(session.account, drawn, now).await.map_err(Stop::stored_codes)?;
        let shown = Minted { codes: codes.iter().map(Code::reveal).collect() };
        Self::json(Status::Created, &shown)
    }

    /// Spends a recovery code to start registering a passkey for the account it belongs to. A code
    /// that is not shaped like one is turned away before it is counted or looked up, so a typo is
    /// not an attempt; the browser is told to drop any session cookie, so the registration that
    /// follows is the ceremony's alone.
    async fn recover(&self, body: &[u8], request: &Request, now: Timestamp) -> Result<Reply, Stop> {
        let asked: Recovering = Unreadable::read(body).map_err(Stop::Body)?;
        let code: Code = asked.code.parse().map_err(Stop::Code)?;
        let label = Label::parse(&asked.label).map_err(Stop::Label)?;
        self.recovery.admit(request.source, now).map_err(Stop::Limited)?;
        let account = self.codes.spend(&code, now).await.map_err(Stop::stored_codes)?.ok_or(Stop::Spent)?;
        let mut reply = self.starting(account, label, now).await?;
        reply.cookie = Some(self.config.session.clear());
        Ok(reply)
    }

    async fn register(&self, request: &Request, body: &[u8], now: Timestamp) -> Result<Reply, Stop> {
        // a session adds a passkey to its own account, once it has stepped up; with no session the
        // ceremony cookie alone is the proof, because only a recovery code starts one without a session
        let session = match self.session(request, now).await {
            Ok(session) => Some(session),
            Err(Stop::Anonymous) if self.held(request).is_some() => None,
            Err(stop) => return Err(stop),
        };
        if let Some(session) = &session {
            self.fresh(session, now).await?;
        }
        let registered: Registered = Unreadable::read(body).map_err(Stop::Body)?;
        let label = Label::parse(&registered.label).map_err(Stop::Label)?;
        let spent = self.spent(request, Flow::Register, now).await?;
        let account = match (&session, spent.account) {
            (Some(session), Some(account)) if session.account == account => account,
            (None, Some(account)) => account,
            _ => return Err(Stop::Challenge),
        };
        let refused = |refusal| Stop::Refused { flow: Flow::Register, refusal };
        let enrolled =
            self.passkeys.finish_registration(&Registering(spent.state), &registered.credential).map_err(refused)?;
        let id = self.drawn()?.factor;
        let fresh = Fresh { id, account, enrolled: &enrolled, label: &label };
        self.registry.add(fresh, self.config.relying.id(), now).await.map_err(Stop::stored)?;
        let mut reply = Self::json(Status::Created, &Added { id: id.to_string(), label: label.to_string() })?;
        reply.ceremony = Some(self.config.ceremony.clear());
        Ok(reply)
    }

    async fn challenge(&self, request: &Request, now: Timestamp) -> Result<Reply, Stop> {
        self.challenge.admit(request.source, now).map_err(Stop::Limited)?;
        let (asking, Authenticating(state)) =
            self.passkeys.start_login().map_err(|error| Stop::Verifier(Box::new(error)))?;
        let drawn = self.drawn()?;
        let mut reply = Self::json(Status::Ok, &asking)?;
        let begin = Begin { flow: Flow::Login, account: None, state: &state, proof: &drawn.proof };
        self.begin(begin, now, &mut reply).await?;
        Ok(reply)
    }

    async fn assert(&self, request: &Request, body: &[u8], now: Timestamp) -> Result<Reply, Stop> {
        let asked: Asking = Unreadable::read(body).map_err(Stop::Body)?;
        let spent = self.spent(request, Flow::Login, now).await?;
        let refused = |refusal| Stop::Refused { flow: Flow::Login, refusal };
        let claim = self.passkeys.who(&asked.credential).map_err(refused)?;
        let found = self.registry.find(&claim.credential).await.map_err(Stop::stored)?;
        // an unknown credential and another account's are one answer
        let passkey = found
            .filter(|passkey| passkey.account == claim.account)
            .ok_or_else(|| refused(Refusal::new(Reason::Credential)))?;
        let verified = self.passkeys.finish_login(&Authenticating(spent.state), &asked.credential, &passkey.stored);
        let verified = match verified {
            Ok(verified) => verified,
            Err(refusal) => {
                if refusal.reason == Reason::Counter {
                    let audit = Audit::Flagged { passkey: passkey.id, refused: true };
                    self.registry.record(&passkey, audit, now).await.map_err(Stop::stored)?;
                }
                return Err(refused(refusal));
            }
        };
        self.registry.used(&passkey, &verified, now).await.map_err(Stop::stored)?;
        // a passkey sign-in is also the step-up, and it replaces the session the browser held: the
        // old secret stops working, so a session id is never kept across a proof of presence
        let replaced = request.cookie.as_deref().and_then(|header| self.config.session.read(header));
        let passed = Passed { account: passkey.account, passkey: passkey.id };
        let entered = self.entry.enter(passed, self.drawn()?, now).await.map_err(Stop::signed)?;
        if let Some(old) = replaced {
            self.entry.end(&old, now).await.map_err(Stop::signed)?;
        }
        self.began(&entered, now)
    }

    /// The reply of a sign-in: when the session ends, and its cookie. No secret is in the body.
    fn began(&self, entered: &Entered, now: Timestamp) -> Result<Reply, Stop> {
        let age = Duration::try_from(entered.session.limit.duration_since(now)).unwrap_or_default();
        let began = Began {
            account: entered.account,
            id: entered.session.id,
            expires: entered.session.expires,
            limit: entered.session.limit,
        };
        let mut reply = Self::json(Status::Ok, &began)?;
        reply.cookie = Some(self.config.session.set(&entered.secret, age));
        reply.ceremony = Some(self.config.ceremony.clear());
        Ok(reply)
    }

    async fn list(&self, request: &Request, now: Timestamp) -> Result<Reply, Stop> {
        let session = self.session(request, now).await?;
        let passkeys = self.registry.list(session.account).await.map_err(Stop::stored)?;
        Self::json(Status::Ok, &passkeys.iter().map(Listed::from).collect::<Vec<_>>())
    }

    async fn remove(
        &self,
        request: &Request,
        id: Option<Id<control::FactorKind>>,
        now: Timestamp,
    ) -> Result<Reply, Stop> {
        let session = self.session(request, now).await?;
        self.fresh(&session, now).await?;
        let id = id.ok_or(Stop::Missing)?;
        self.registry.remove(session.account, id, now).await.map_err(Stop::stored)?;
        let mut reply = Self::json(Status::Ok, &Removed { removed: true })?;
        // a session the removed passkey made ends with it, and the browser is told
        if self.session(request, now).await.is_err() {
            reply.cookie = Some(self.config.session.clear());
        }
        Ok(reply)
    }
}

impl<D: Db + Clone, K: Clock, R: Draw, P: Passkeys> Answers for Ceremonies<D, K, R, P> {
    fn answer(&self, request: &Request) -> impl std::future::Future<Output = Reply> {
        self.respond(request)
    }
}

impl<D: Db + Clone, K: Clock, R: Draw, P: Passkeys> Owner for Ceremonies<D, K, R, P> {
    fn owns(&self, path: &str) -> bool {
        Step::owns(path)
    }
}
