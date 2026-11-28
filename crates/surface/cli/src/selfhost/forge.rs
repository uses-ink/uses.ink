//! How a self-host reaches forges and takes their webhooks (decisions 0160 to 0162): off unless the
//! operator turns it on, because every forge call is a request the server sends, and every webhook
//! is a body from a stranger. Each value has a flag, an `INK_` variable and a default.

use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use authority_forge::{Host, Limits};
use authority_forgejo::Instances;
use authority_repo::{Forges, Repos, Wire};
use clap::Args;
use https::{Fetch, Https, Route};
use jiff::SignedDuration;

use crate::grammar::tree::Heading;

/// A name sent to an address instead of the one DNS gives (`--forge-route`): `HOST=ADDRESS:PORT`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapped(Route);

/// Text that is no `HOST=ADDRESS:PORT`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unmapped {
    /// What was written.
    pub written: String,
}

impl std::fmt::Display for Unmapped {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "`{}` is not HOST=ADDRESS:PORT", self.written)
    }
}

impl std::error::Error for Unmapped {}

impl FromStr for Mapped {
    type Err = Unmapped;

    fn from_str(text: &str) -> Result<Self, Unmapped> {
        let unmapped = || Unmapped { written: text.to_owned() };
        let (host, address) = text.split_once('=').ok_or_else(unmapped)?;
        host.parse::<Host>().map_err(|_| unmapped())?;
        Ok(Self(Route { host: host.to_owned(), address: address.parse().map_err(|_| unmapped())? }))
    }
}

/// A setting that cannot be used.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum Misforged {
    /// The file of instances cannot be read.
    Read {
        /// The file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The file of instances is not a list of instances.
    Instances {
        /// The file.
        path: PathBuf,
        /// Why.
        source: authority_forgejo::Unreadable,
    },
    /// The built-in list of instances is not valid.
    Bundled(authority_forgejo::Unreadable),
    /// The https client could not be built.
    Client(https::Misconfigured),
}

impl std::fmt::Display for Misforged {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, .. } => write!(out, "{} could not be read", path.display()),
            Self::Instances { path, .. } => write!(out, "{} is not a list of forge instances", path.display()),
            Self::Bundled(_) => out.write_str("the built-in list of forge instances is not valid"),
            Self::Client(_) => out.write_str("the client that reaches forges could not be built"),
        }
    }
}

impl std::error::Error for Misforged {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Instances { source, .. } | Self::Bundled(source) => Some(source),
            Self::Client(source) => Some(source),
        }
    }
}

/// The settings of forge calls and webhooks.
#[derive(Clone, Debug, Args)]
pub struct Forging {
    /// Take webhooks from forges at `POST /v1/hooks/{github,forgejo,gitlab}`, and let a site claim
    /// and prove its repository, which the server reads from the forge over https. Off by default.
    #[arg(long = "forge-hooks", env = "INK_FORGE_HOOKS", help_heading = Heading::HOSTING)]
    pub hooks: bool,
    /// A TOML file of the Forgejo instances this server talks to, in the format of the built-in list
    /// (Codeberg). A repository on a host that is on no list is refused before anything is sent.
    #[arg(long = "forge-instances", value_name = "FILE", env = "INK_FORGE_INSTANCES", help_heading = Heading::HOSTING)]
    pub instances: Option<PathBuf>,
    /// The GitLab hosts this server talks to; repeat or separate with commas. [default: gitlab.com]
    #[arg(long = "forge-gitlab", value_name = "HOST", env = "INK_FORGE_GITLAB", value_delimiter = ',', help_heading = Heading::HOSTING)]
    pub gitlab: Vec<Host>,
    /// Send a forge's name to an address instead of the one DNS gives, as `HOST=ADDRESS:PORT`: for
    /// a mirror, or a test. The certificate is still checked for the name.
    #[arg(long = "forge-route", value_name = "HOST=ADDR", env = "INK_FORGE_ROUTES", value_delimiter = ',', help_heading = Heading::HOSTING)]
    pub routes: Vec<Mapped>,
    /// Seconds one call to a forge may take in all. [default: 10]
    #[arg(long = "forge-timeout", value_name = "SECONDS", env = "INK_FORGE_TIMEOUT", help_heading = Heading::HOSTING)]
    pub timeout: Option<u64>,
    /// The most bytes of a forge's reply that are read. [default: 1048576]
    #[arg(long = "forge-reply", value_name = "BYTES", env = "INK_FORGE_REPLY", help_heading = Heading::HOSTING)]
    pub reply: Option<usize>,
    /// The most bytes of a webhook body that are read. [default: 5242880]
    #[arg(long = "forge-body", value_name = "BYTES", env = "INK_FORGE_BODY", help_heading = Heading::HOSTING)]
    pub body: Option<usize>,
    /// Seconds a delivery id is remembered, so the same delivery is taken once. [default: 600]
    #[arg(long = "forge-window", value_name = "SECONDS", env = "INK_FORGE_WINDOW", help_heading = Heading::HOSTING)]
    pub window: Option<u64>,
    /// How many delivery ids of one repository are remembered at once. [default: 10000]
    #[arg(long = "forge-remembered", value_name = "N", env = "INK_FORGE_REMEMBERED", help_heading = Heading::HOSTING)]
    pub remembered: Option<usize>,
}

impl Forging {
    /// The limits of a forge authority: the flags over the defaults.
    pub(crate) fn limits(&self) -> Limits {
        let defaults = Limits::DEFAULT;
        Limits {
            body: self.body.unwrap_or(defaults.body),
            window: self.window.map_or(defaults.window, |seconds| SignedDuration::from_secs(seconds.cast_signed())),
            remembered: self.remembered.unwrap_or(defaults.remembered),
            reply: self.reply.unwrap_or(defaults.reply),
            timeout: self.timeout.map_or(defaults.timeout, Duration::from_secs),
        }
    }

    /// The forges this server reaches, and how far.
    fn forges(&self) -> Result<Forges, Misforged> {
        let forgejo = match &self.instances {
            Some(path) => {
                let text =
                    std::fs::read_to_string(path).map_err(|source| Misforged::Read { path: path.clone(), source })?;
                Instances::read(&text).map_err(|source| Misforged::Instances { path: path.clone(), source })?
            }
            None => Instances::bundled().map_err(Misforged::Bundled)?,
        };
        let gitlab = if self.gitlab.is_empty() { vec![Host::GITLAB] } else { self.gitlab.clone() };
        Ok(Forges { forgejo, gitlab, limits: self.limits() })
    }

    /// The authority over the real transport when forges are on, reaching only the hosts the
    /// operator listed: GitHub's API, each Forgejo instance and each GitLab host.
    pub(crate) fn repos(&self, https: &Https) -> Result<Option<Repos<Wire>>, Misforged> {
        if !self.hooks {
            return Ok(None);
        }
        let forges = self.forges()?;
        let mut hosts = vec![Host::API];
        hosts.extend(forges.forgejo.all().iter().map(|instance| instance.host.clone()));
        hosts.extend(forges.gitlab.iter().cloned());
        let routes: Vec<Route> = self.routes.iter().map(|mapped| mapped.0.clone()).collect();
        let fetch = Fetch::new(https, &routes).map_err(Misforged::Client)?;
        let agent = format!("uses-ink/{}", env!("CARGO_PKG_VERSION"));
        Ok(Some(Repos::new(forges, Wire::new(fetch, hosts, agent))))
    }
}
