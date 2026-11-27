//! The server's settings for keys and signatures: flags and environment variables, each with a
//! default `ink serve --data` prints when it starts (decision 0068).

use std::net::SocketAddr;

use clap::Args;
use control::keys::Settings;
use control::signin::Ttl;
use edge_api::{Closing, Signing, Window};
use sig::Audience;

use crate::product::Unready;
use crate::tree::Heading;

/// The flags of keys, signatures and enrolment.
#[derive(Debug, Args)]
pub struct Flags {
    /// The authority a signature is made for, as a client writes it: `host[:port]` of the API.
    /// Not the `Host` header, so a proxy cannot choose it. [default: `--api-host` and the port of
    /// `--addr`]
    #[arg(long, env = "INK_AUDIENCE", help_heading = Heading::KEYS)]
    pub audience: Option<String>,
    /// Seconds either side of the server's time a signed request may be, and the quiet after a
    /// restart. At most 300. [default: 60]
    #[arg(long, env = "INK_AUTH_SKEW", help_heading = Heading::KEYS)]
    pub auth_skew: Option<u64>,
    /// How many request nonces are remembered for one key at once. [default: 1024]
    #[arg(long, env = "INK_AUTH_NONCES", help_heading = Heading::KEYS)]
    pub auth_nonces: Option<usize>,
    /// How long a device key lives. [default: 180d]
    #[arg(long, env = "INK_KEY_DEVICE", help_heading = Heading::KEYS)]
    pub key_device: Option<Ttl>,
    /// How long a deploy key lives. [default: 90d]
    #[arg(long, env = "INK_KEY_DEPLOY", help_heading = Heading::KEYS)]
    pub key_deploy: Option<Ttl>,
    /// The longest a key may be asked to live. [default: 365d]
    #[arg(long, env = "INK_KEY_CEILING", help_heading = Heading::KEYS)]
    pub key_ceiling: Option<Ttl>,
    /// How long a person has to approve a key. [default: 10m]
    #[arg(long, env = "INK_ENROL_TTL", help_heading = Heading::KEYS)]
    pub enrol_ttl: Option<Ttl>,
    /// How many wrong codes close enrolling. [default: 5]
    #[arg(long, env = "INK_ENROL_GUESSES", help_heading = Heading::KEYS)]
    pub enrol_guesses: Option<u32>,
    /// How many keys may wait for approval at once. [default: 20]
    #[arg(long, env = "INK_ENROL_WAITING", help_heading = Heading::KEYS)]
    pub enrol_waiting: Option<u64>,
    /// How long enrolling stays closed. [default: 1m]
    #[arg(long, env = "INK_ENROL_CLOSED", help_heading = Heading::KEYS)]
    pub enrol_closed: Option<Ttl>,
}

/// What the flags came to, with every default filled in, for the server and for its start-up
/// listing.
#[derive(Clone, Debug)]
pub(crate) struct Resolved {
    pub(crate) keys: Settings,
    pub(crate) closing: Closing,
    pub(crate) window: Window,
    pub(crate) nonces: usize,
    pub(crate) audience: Audience,
}

impl Flags {
    /// The settings, over the defaults. `api_host` and `address` give the audience when none is
    /// set: the port is left out when it is the one `http` has.
    pub(crate) fn resolve(&self, api_host: &str, address: SocketAddr) -> Result<Resolved, Unready> {
        let setting = |flag: &'static str| move |source: Box<dyn std::error::Error + Send + Sync>| Unready::Setting { flag, source };
        let defaults = Settings::default();
        let keys = Settings {
            device: self.key_device.unwrap_or(defaults.device),
            deploy: self.key_deploy.unwrap_or(defaults.deploy),
            ceiling: self.key_ceiling.unwrap_or(defaults.ceiling),
            enrol: self.enrol_ttl.unwrap_or(defaults.enrol),
            kept: defaults.kept,
        };
        let rules = Closing::default();
        let closing = Closing {
            guesses: self.enrol_guesses.unwrap_or(rules.guesses),
            waiting: self.enrol_waiting.unwrap_or(rules.waiting),
            closed: self.enrol_closed.unwrap_or(rules.closed),
        };
        let window = Window::new(self.auth_skew.unwrap_or(Window::DEFAULT.seconds()))
            .map_err(|e| setting("--auth-skew")(Box::new(e)))?;
        let written = self.audience.clone().unwrap_or_else(|| match address.port() {
            80 => api_host.to_owned(),
            port => format!("{api_host}:{port}"),
        });
        let audience = Audience::parse(&written).map_err(|e| setting("--audience")(Box::new(e)))?;
        Ok(Resolved { keys, closing, window, nonces: self.auth_nonces.unwrap_or(Signing::NONCES), audience })
    }
}
