//! Verifying a GitLab webhook delivery.

use jiff::Timestamp;
use serde::Deserialize;

use authority_forge::{Headers, Host, Ignored, Intake, Limits, Push, Received, Refused, Repo, Roster, Secret};

/// The part of a push event this reads. Every other field is left unread.
#[derive(Debug, Deserialize)]
struct Event {
    #[serde(rename = "ref")]
    reference: String,
    after: String,
    project: Project,
}

#[derive(Debug, Deserialize)]
struct Project {
    path_with_namespace: String,
    #[serde(default)]
    web_url: String,
    #[serde(default)]
    default_branch: Option<String>,
}

/// What `Hooks::named` reads: the project alone.
#[derive(Debug, Deserialize)]
struct Naming {
    project: Project,
}

/// The webhook of one GitLab instance: the secret token the hook was made with.
#[derive(Debug)]
pub struct Hooks {
    host: Host,
    secret: Secret,
    intake: Intake,
}

impl Hooks {
    /// The header that carries the secret token itself. GitLab does not sign the body, so
    /// whoever sees one delivery on the wire sees the token: the hook must be `https`.
    pub const TOKEN: &'static str = "X-Gitlab-Token";
    /// The header with the kind of event: `Push Hook` for a push.
    pub const EVENT: &'static str = "X-Gitlab-Event";
    /// The headers with the delivery id: the event's UUID, or the idempotency key older
    /// versions send.
    pub const DELIVERY: (&'static str, &'static str) = ("X-Gitlab-Event-UUID", "Idempotency-Key");

    /// The repository a delivery names, with the instance read from the project's `web_url`,
    /// before the delivery is trusted: it finds the token the delivery must carry. Nothing it
    /// returns is acted on until the token held.
    pub fn named(body: &[u8]) -> Result<Repo, Refused> {
        let naming: Naming = serde_json::from_slice(body).map_err(Refused::Json)?;
        Push::repo(Push::host(&naming.project.web_url)?, &naming.project.path_with_namespace)
    }

    /// A webhook of the instance at `host` that checks deliveries against `secret` within `limits`.
    pub fn new(host: Host, secret: Secret, limits: Limits) -> Self {
        Self { host, secret, intake: Intake::new(limits) }
    }

    /// Checks one delivery and says what it comes to, in the order of the size of the body, the
    /// token (compared in constant time over digests, so not even its length leaks), the replay
    /// window, the kind of event and the JSON.
    pub fn receive(&self, delivery: (&Headers, &[u8]), roster: &Roster, now: Timestamp) -> Result<Received, Refused> {
        let (headers, body) = delivery;
        self.intake.size(body)?;
        if !self.secret.matches(headers.need(Self::TOKEN)?.as_bytes()) {
            return Err(Refused::Signature);
        }
        let event = headers.need(Self::EVENT)?;
        self.intake.admit(headers.either(Self::DELIVERY)?, now)?;
        if event != "Push Hook" {
            return Ok(Received::Ignored(Ignored::Event { written: event.to_owned() }));
        }
        let pushed: Event = serde_json::from_slice(body).map_err(Refused::Json)?;
        let repo = Push::repo(self.host.clone(), &pushed.project.path_with_namespace)?;
        Push::read(repo, (&pushed.reference, &pushed.after, pushed.project.default_branch.as_deref()), roster)
    }
}
