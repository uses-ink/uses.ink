//! Verifying a GitHub webhook delivery.

use jiff::Timestamp;
use serde::Deserialize;

use authority_forge::{Headers, Host, Ignored, Intake, Limits, Push, Received, Refused, Repo, Roster, Secret};

/// The part of a push event this reads. Every other field is left unread.
#[derive(Debug, Deserialize)]
struct Event {
    #[serde(rename = "ref")]
    reference: String,
    after: String,
    repository: Named,
}

#[derive(Debug, Deserialize)]
struct Named {
    full_name: String,
    #[serde(default)]
    default_branch: Option<String>,
}

/// What `Hooks::named` reads: the repository alone.
#[derive(Debug, Deserialize)]
struct Naming {
    repository: Named,
}

/// The webhook of the App: one secret for every delivery.
#[derive(Debug)]
pub struct Hooks {
    secret: Secret,
    intake: Intake,
}

impl Hooks {
    /// The header with the signature: `sha256=` and the hex HMAC-SHA256 of the body.
    pub const SIGNATURE: &'static str = "X-Hub-Signature-256";
    /// The header with the kind of event.
    pub const EVENT: &'static str = "X-GitHub-Event";
    /// The header with the delivery id, which the signature does not cover.
    pub const DELIVERY: &'static str = "X-GitHub-Delivery";

    /// The repository a delivery names, read before the delivery is trusted, to find the secret
    /// it must be signed with. Nothing it returns is acted on until the signature held.
    pub fn named(body: &[u8]) -> Result<Repo, Refused> {
        let naming: Naming = serde_json::from_slice(body).map_err(Refused::Json)?;
        Push::repo(Host::GITHUB, &naming.repository.full_name)
    }

    /// A webhook that checks deliveries against `secret` within `limits`.
    pub fn new(secret: Secret, limits: Limits) -> Self {
        Self { secret, intake: Intake::new(limits) }
    }

    /// Checks one delivery and says what it comes to. The order is the size of the body, the
    /// signature over the raw bytes, the delivery id against the replay window, the kind of
    /// event, and only then the JSON. A refusal says which check failed and no more.
    pub fn receive(&self, delivery: (&Headers, &[u8]), roster: &Roster, now: Timestamp) -> Result<Received, Refused> {
        let (headers, body) = delivery;
        self.intake.size(body)?;
        let tag = headers.need(Self::SIGNATURE)?.strip_prefix("sha256=").ok_or(Refused::Signature)?;
        if !self.secret.authenticates(body, tag) {
            return Err(Refused::Signature);
        }
        let event = headers.need(Self::EVENT)?;
        self.intake.admit(headers.need(Self::DELIVERY)?, now)?;
        if event != "push" {
            return Ok(Received::Ignored(Ignored::Event { written: event.to_owned() }));
        }
        let pushed: Event = serde_json::from_slice(body).map_err(Refused::Json)?;
        let repo = Push::repo(Host::GITHUB, &pushed.repository.full_name)?;
        Push::read(repo, (&pushed.reference, &pushed.after, pushed.repository.default_branch.as_deref()), roster)
    }
}
