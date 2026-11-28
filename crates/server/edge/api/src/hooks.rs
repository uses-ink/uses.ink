//! A forge's webhook (decision 0161): `POST /v1/hooks/{forge}` tells the server a repository a site
//! proved has moved, and the site builds.
//!
//! Nothing here is signed with a key of ours: the forge signs the body with the secret of the
//! claim, so the order is the forge's own. The body is read for the repository it names, the claim
//! of that repository gives the secret, the forge crate checks size, signature, event and JSON in
//! its order, and only then is the delivery id claimed in the [`Nonces`] store, so only a sender who
//! holds the secret takes room in it. A delivery that is a push goes to [`watch::Watch::pushed`],
//! which queues the build through the dedupe the poll uses.

use std::collections::BTreeMap;

use authority_forge::{Headers, Limits, Received, Refused, Repo, Roster, Secret};
use authority_forgejo::Hooks as Forgejo;
use authority_github::Hooks as Github;
use authority_gitlab::Hooks as Gitlab;
use authority_repo::Forge;
use clock::{Clock, Draw};
use control::{Control, Ports};
use jiff::Timestamp;
use serde::Serialize;
use sig::{KeyId, Nonce};
use watch::{Head, HeadError, Outcome, Previews, Pushed, Watch};

use crate::nonce::{Claimed, Claiming, Nonces};
use crate::port::{Port, Unnamed};
use crate::reply::{Media, Method, Payload, Reply, Request, Status};
use crate::view::Problem;
use crate::{Answers, Owner};

/// The path every webhook is under.
pub const HOOKS: &str = "/v1/hooks/";

/// What a delivery that was taken is answered with.
#[derive(Debug, Serialize)]
pub struct Told {
    /// `queued`, `duplicate`, `left` or `ignored`.
    pub outcome: &'static str,
    /// The build that was queued.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// Why nothing was queued.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// Why a delivery was not taken.
enum Declined<P: Ports> {
    /// Not a `POST`.
    Method,
    /// The path names no forge.
    Forge(authority_repo::Unknown),
    /// The body is over the limit.
    Large(u64),
    /// The forge crate refused it.
    Refused(Refused),
    /// No proven repository is named by the delivery: answered as a bad signature is, so a
    /// stranger cannot tell which repositories are followed.
    Unfollowed,
    /// The delivery id was seen.
    Replayed,
    /// The key of the repository has as many ids remembered as are kept.
    Flooded {
        /// The most kept.
        most: usize,
    },
    /// The store has not been up for the quiet period.
    Quiet {
        /// Seconds until it has.
        seconds: u64,
    },
    /// The commit is no commit id.
    Head(HeadError),
    /// The control plane failed.
    Control(control::Error<P>),
    /// The nonce store failed.
    Nonces(Unnamed),
}

impl<P: Ports> Declined<P> {
    /// The catalog code.
    const fn code(&self) -> &'static str {
        match self {
            Self::Method | Self::Forge(_) => "route",
            Self::Large(_) => "hook.body.large",
            Self::Refused(refused) => refused.code(),
            Self::Unfollowed => "hook.signature.invalid",
            Self::Replayed => "hook.delivery.replayed",
            Self::Flooded { .. } => "auth.signature.flooded",
            Self::Quiet { .. } => "auth.server.restarting",
            Self::Head(_) => "hook.payload.malformed",
            Self::Control(_) | Self::Nonces(_) => "control",
        }
    }

    /// The status.
    const fn status(&self) -> Status {
        match self {
            Self::Method => Status::MethodNotAllowed,
            Self::Forge(_) => Status::NotFound,
            Self::Large(_) | Self::Refused(Refused::Oversize { .. }) => Status::TooLarge,
            Self::Refused(Refused::Signature) | Self::Unfollowed => Status::Unauthorized,
            Self::Refused(Refused::Replayed { .. }) | Self::Replayed => Status::Conflict,
            Self::Refused(_) | Self::Head(_) => Status::BadRequest,
            Self::Flooded { .. } => Status::TooMany,
            Self::Quiet { .. } => Status::Unavailable,
            Self::Control(_) | Self::Nonces(_) => Status::Internal,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Method => "a webhook is a POST".to_owned(),
            Self::Forge(error) => error.to_string(),
            Self::Large(limit) => format!("the body is over the limit of {limit} bytes"),
            Self::Refused(refused) => refused.to_string(),
            Self::Unfollowed => "the delivery's signature does not authenticate its body".to_owned(),
            Self::Replayed => "the delivery was seen already".to_owned(),
            Self::Flooded { most } => format!("{most} deliveries of this repository are remembered at once"),
            Self::Quiet { seconds } => format!("the server restarted a moment ago: try again in {seconds} seconds"),
            Self::Head(error) => error.to_string(),
            Self::Control(_) => "the control plane did not answer".to_owned(),
            Self::Nonces(error) => error.to_string(),
        }
    }

    fn reply(&self) -> Reply {
        let mut fields = BTreeMap::new();
        match self {
            Self::Quiet { seconds } => {
                fields.insert("seconds", seconds.to_string());
            }
            Self::Flooded { most } => {
                fields.insert("window", most.to_string());
            }
            _ => {}
        }
        let problem = Problem { problem: self.code(), message: self.message(), causes: Vec::new(), fields };
        Reply::new(self.status(), Media::Json, serde_json::to_vec(&problem).unwrap_or_default())
    }
}

/// Bytes of a key or a nonce, derived from text, so a store keyed for signed requests holds
/// deliveries too.
struct Digest;

impl Digest {
    const KEY: &'static str = "uses.ink 2026-11-28 webhook repository v1";
    const NONCE: &'static str = "uses.ink 2026-11-28 webhook delivery v1";

    fn of(context: &str, text: &str) -> [u8; 16] {
        let derived = blake3::derive_key(context, text.as_bytes());
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(derived.get(..16).unwrap_or_default());
        bytes
    }
}

/// The webhook endpoint, over a control plane, a nonce store, a clock and a source of ids.
#[derive(Debug)]
pub struct Hooks<P: Ports, N, K, D> {
    control: Control<P>,
    nonces: N,
    clocks: (K, D),
    limits: Limits,
    previews: Previews,
    on: bool,
}

impl<P: Ports + 'static, N: Nonces, K: Clock, D: Draw> Hooks<P, N, K, D> {
    /// The endpoint of `control`, remembering deliveries in `nonces`, within `limits` (the body, the
    /// window ids are remembered for and how many are), and building up to `previews` branches
    /// of a push to a branch that is not the site's own.
    pub const fn new(control: Control<P>, nonces: N, clocks: (K, D), setting: (Limits, Previews)) -> Self {
        Self { control, nonces, clocks, limits: setting.0, previews: setting.1, on: true }
    }

    /// The same endpoint, owning its path only when `on`: a server that was not asked to take
    /// webhooks answers the path as it answers any path that is no route.
    #[must_use]
    pub fn enabled(self, on: bool) -> Self {
        Self { on, ..self }
    }

    /// The control plane the endpoint answers from.
    pub const fn control(&self) -> &Control<P> {
        &self.control
    }

    /// What a delivery comes to once the secret of its repository's claim has checked it.
    fn verify(
        &self,
        forge: Forge,
        held: (&Repo, Secret),
        seen: (&Headers, &[u8], Timestamp),
    ) -> Result<Received, Refused> {
        let (repo, secret) = held;
        let (headers, body, now) = seen;
        let roster = Roster::from_iter([repo.clone()]);
        let delivery = (headers, body);
        match forge {
            Forge::Github => Github::new(secret, self.limits).receive(delivery, &roster, now),
            Forge::Forgejo => Forgejo::new(repo.host.clone(), secret, self.limits).receive(delivery, &roster, now),
            Forge::Gitlab => Gitlab::new(repo.host.clone(), secret, self.limits).receive(delivery, &roster, now),
        }
    }

    async fn take(&self, request: &Request) -> Result<(Forge, Told), Declined<P>> {
        let forge: Forge = request.path.strip_prefix(HOOKS).unwrap_or_default().parse().map_err(Declined::Forge)?;
        if request.method != Method::Post {
            return Err(Declined::Method);
        }
        let body = match &request.body {
            Payload::Whole(body) => body.as_slice(),
            Payload::Over { limit } => return Err(Declined::Large(*limit)),
        };
        let headers = request.headers.iter().fold(Headers::new(), |held, (name, value)| held.with(name, value));
        let repo = match forge {
            Forge::Github => Github::named(body),
            Forge::Forgejo => Forgejo::named(body),
            Forge::Gitlab => Gitlab::named(body),
        }
        .map_err(Declined::Refused)?;
        let claim = self.control.bound(&repo).await.map_err(Declined::Control)?.ok_or(Declined::Unfollowed)?;
        let plain = self.control.hook_secret(&claim).await.map_err(Declined::Control)?;
        let secret = Secret::new(plain.expose().to_vec());
        let now = self.clocks.0.now();
        let received = self.verify(forge, (&repo, secret), (&headers, body, now)).map_err(Declined::Refused)?;
        self.remember(forge, (&repo, &headers), now).await?;
        let told = match received {
            Received::Ignored(ignored) => Told { outcome: "ignored", build: None, why: Some(ignored.to_string()) },
            Received::Push(push) => {
                let head: Head = push.revision.as_str().parse().map_err(Declined::Head)?;
                let pushed = Pushed { branch: push.branch, head, default: push.default };
                let watch = Watch::pusher(&self.control, (&self.clocks.0, &self.clocks.1), self.previews);
                Self::told(watch.pushed(claim.site, &pushed).await)
            }
            _ => Told { outcome: "ignored", build: None, why: Some("the delivery is not a push".to_owned()) },
        };
        Ok((forge, told))
    }

    /// Claims the delivery id of the headers in the nonce store, which is the replay memory: the
    /// forge crate's own window lives only as long as one delivery.
    async fn remember(&self, forge: Forge, seen: (&Repo, &Headers), now: Timestamp) -> Result<(), Declined<P>> {
        let (repo, headers) = seen;
        let id = match forge {
            Forge::Github => headers.need(Github::DELIVERY),
            Forge::Forgejo => headers.either(Forgejo::DELIVERY),
            Forge::Gitlab => headers.either(Gitlab::DELIVERY),
        }
        .map_err(Declined::Refused)?;
        let seconds = u64::try_from(now.as_second()).unwrap_or_default();
        let window = u64::try_from(self.limits.window.as_secs()).unwrap_or_default();
        let claiming = Claiming {
            key: KeyId::new(Digest::of(Digest::KEY, &format!("{forge} {repo}"))),
            nonce: Nonce::new(Digest::of(Digest::NONCE, id)),
            until: seconds.saturating_add(window),
            now: seconds,
            most: self.limits.remembered,
        };
        match self.nonces.claim(claiming).await.map_err(|error| Declined::Nonces(Unnamed::new(Port::Nonces, error)))? {
            Claimed::Fresh => Ok(()),
            Claimed::Replay => Err(Declined::Replayed),
            Claimed::Full => Err(Declined::Flooded { most: self.limits.remembered }),
            Claimed::Quiet { seconds } => Err(Declined::Quiet { seconds }),
        }
    }

    fn told(outcome: Outcome) -> Told {
        match outcome {
            Outcome::Queued { build, .. } => Told { outcome: "queued", build: Some(build.to_string()), why: None },
            Outcome::Left(why) => Told { outcome: "left", build: None, why: Some(why.to_string()) },
            Outcome::Failed(error) => Told { outcome: "failed", build: None, why: Some(error.to_string()) },
            _ => Told { outcome: "duplicate", build: None, why: None },
        }
    }
}

impl<P: Ports + 'static, N: Nonces, K: Clock, D: Draw> Answers for Hooks<P, N, K, D> {
    async fn answer(&self, request: &Request) -> Reply {
        match self.take(request).await {
            Ok((forge, told)) => {
                let status = if told.outcome == "failed" { Status::Internal } else { Status::Accepted };
                logging::info!(name: logging::event::HOOK_RECEIVED, forge = forge.name(), outcome = told.outcome);
                Reply::new(status, Media::Json, serde_json::to_vec(&told).unwrap_or_default())
            }
            Err(declined) => {
                logging::warn!(name: logging::event::HOOK_RECEIVED, outcome = "refused", code = declined.code());
                declined.reply()
            }
        }
    }
}

impl<P: Ports + 'static, N: Nonces, K: Clock, D: Draw> Owner for Hooks<P, N, K, D> {
    fn owns(&self, path: &str) -> bool {
        self.on && path.starts_with(HOOKS)
    }
}
