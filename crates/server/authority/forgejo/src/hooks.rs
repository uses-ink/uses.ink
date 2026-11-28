//! Verifying a Forgejo or Gitea webhook delivery.

use jiff::Timestamp;
use serde::Deserialize;

use authority_forge::{Headers, Host, Ignored, Intake, Limits, Push, Received, Refused, Roster, Secret};

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
}

/// The webhook of one instance: the secret that instance signs with.
#[derive(Debug)]
pub struct Hooks {
    host: Host,
    secret: Secret,
    intake: Intake,
}

impl Hooks {
    /// The headers with the signature: the hex HMAC-SHA256 of the body, with no prefix. Forgejo
    /// names it its own way and keeps Gitea's.
    pub const SIGNATURE: (&'static str, &'static str) = ("X-Forgejo-Signature", "X-Gitea-Signature");
    /// The headers with the kind of event.
    pub const EVENT: (&'static str, &'static str) = ("X-Forgejo-Event", "X-Gitea-Event");
    /// The headers with the delivery id, which the signature does not cover.
    pub const DELIVERY: (&'static str, &'static str) = ("X-Forgejo-Delivery", "X-Gitea-Delivery");

    /// A webhook of the instance at `host` that checks deliveries against `secret` within `limits`.
    pub fn new(host: Host, secret: Secret, limits: Limits) -> Self {
        Self { host, secret, intake: Intake::new(limits) }
    }

    /// Checks one delivery and says what it comes to, in the order of the size of the body, the
    /// signature over the raw bytes, the replay window, the kind of event and the JSON.
    pub fn receive(&self, delivery: (&Headers, &[u8]), roster: &Roster, now: Timestamp) -> Result<Received, Refused> {
        let (headers, body) = delivery;
        self.intake.size(body)?;
        if !self.secret.authenticates(body, headers.either(Self::SIGNATURE)?) {
            return Err(Refused::Signature);
        }
        let event = headers.either(Self::EVENT)?;
        self.intake.admit(headers.either(Self::DELIVERY)?, now)?;
        if event != "push" {
            return Ok(Received::Ignored(Ignored::Event { written: event.to_owned() }));
        }
        let pushed: Event = serde_json::from_slice(body).map_err(Refused::Json)?;
        let repo = Push::repo(self.host.clone(), &pushed.repository.full_name)?;
        Push::read(repo, (&pushed.reference, &pushed.after), roster)
    }
}
