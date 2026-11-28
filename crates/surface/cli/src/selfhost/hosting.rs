//! How a self-host runs beside the edge: what the lookup keeps, how the watch and the worker
//! behave, how they are restarted, and what git they may reach. Every value has a flag and a
//! default.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, ValueEnum};
use control::{Limits, Policy};
use https::{Follow, Https, Misconfigured};
use model::Bytes;
use model::git::{Network, Scheme};

use crate::grammar::tree::Heading;
use crate::selfhost::tasks::Restart;

/// A kind of git address the operator allows (`--git-scheme`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Allowed {
    /// `https://host/path`.
    Https,
    /// `file:///path` on this machine: for a self-host that builds from its own disk.
    File,
}

impl From<Allowed> for Scheme {
    fn from(allowed: Allowed) -> Self {
        match allowed {
            Allowed::Https => Self::Https,
            Allowed::File => Self::File,
        }
    }
}

/// Which redirects an https remote's answers are followed through (`--git-redirects`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Redirects {
    /// None.
    Never,
    /// Those of the first request of a connection, as `git` does.
    First,
    /// Those of every request.
    Always,
}

impl From<Redirects> for Follow {
    fn from(redirects: Redirects) -> Self {
        match redirects {
            Redirects::Never => Self::Never,
            Redirects::First => Self::First,
            Redirects::Always => Self::Always,
        }
    }
}

/// The settings of the loops and lookups `ink serve --data` runs. Each flag says its default.
#[derive(Clone, Debug, Args)]
pub struct Hosting {
    /// Seconds a site's live deploy is remembered before the control plane is asked again: a new
    /// deploy or a rollback shows within this long. The default asks every time, since the lookup
    /// and the control plane share one process and one database, so there is no staleness to
    /// trade for speed; raise it only to spare the database a read per request. [default: 0]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub site_ttl: Option<u64>,
    /// How many decoded deploys the lookup keeps. [default: 64]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub site_deploys: Option<usize>,
    /// Seconds before the watch looks again for a site it has not seen move, at most; so a new
    /// site is first polled within this long. At least 1. [default: 5]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub poll_lookout: Option<u64>,
    /// Seconds a worker holds a build request before the queue gives it to another. [default: 900]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub worker_lease: Option<u64>,
    /// Seconds a worker rests when the queue is empty. [default: 2]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub worker_idle: Option<u64>,
    /// How many times in a row the watch or the worker may crash before it is left stopped.
    /// [default: 5]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub restart_limit: Option<u32>,
    /// Seconds to wait before a crashed loop is started again. [default: 2]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub restart_delay: Option<u64>,
    /// Seconds a loop must run for a crash after it to count as the first again. [default: 60]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub restart_healthy: Option<u64>,
    /// A kind of git address a site may be added with and the builder fetches; repeat for more.
    /// [default: https]
    #[arg(long = "git-scheme", value_enum, help_heading = Heading::HOSTING)]
    pub git_schemes: Vec<Allowed>,
    /// Where clones of git sites are kept. [default: <data>/git]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub git_cache: Option<PathBuf>,
    /// Seconds a fetch and read of a git source may take. [default: 120]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub git_time: Option<u64>,
    /// Seconds to wait for a connection to an https git remote. [default: 10]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub git_connect: Option<u64>,
    /// Seconds one https request to a git remote may take in all, its body included: also the
    /// longest a pack may take to arrive. [default: 120]
    #[arg(long, help_heading = Heading::HOSTING)]
    pub git_request: Option<u64>,
    /// Which redirects of an https git remote are followed; one that leaves https never is.
    /// [default: first]
    #[arg(long, value_enum, help_heading = Heading::HOSTING)]
    pub git_redirects: Option<Redirects>,
    /// A PEM file of certificate authorities to trust beside the bundled web roots, for a forge
    /// with a private authority; repeat for more. Certificates are always verified.
    #[arg(long = "git-ca", help_heading = Heading::HOSTING)]
    pub git_cas: Vec<PathBuf>,
    /// Days a handle is held after its site left it, so no one else can claim it; a verified
    /// owner's handle is held for good. [default: 365]
    #[arg(long, value_name = "DAYS", env = "INK_HANDLE_HOLD", help_heading = Heading::LIMITS)]
    pub handle_hold: Option<u64>,
    /// Bytes an account may store across its sites; a deploy over it is refused whole.
    /// [default: 1073741824]
    #[arg(long, value_name = "BYTES", env = "INK_LIMIT_STORED", help_heading = Heading::LIMITS)]
    pub limit_stored: Option<u64>,
    /// Builds an account may start in 24 hours; a request over it is refused. [default: 60]
    #[arg(long, value_name = "N", env = "INK_LIMIT_BUILDS", help_heading = Heading::LIMITS)]
    pub limit_builds: Option<u64>,
    /// Sites an account may have. [default: 10]
    #[arg(long, value_name = "N", env = "INK_LIMIT_SITES", help_heading = Heading::LIMITS)]
    pub limit_sites: Option<u64>,
    /// Deploys of a site that are kept, the live one included; older ones are removed with the
    /// files only they used. [default: 20]
    #[arg(long, value_name = "N", env = "INK_LIMIT_DEPLOYS", help_heading = Heading::LIMITS)]
    pub limit_deploys: Option<u64>,
    /// The most blobs one step of removing a site deletes. A removal runs in steps this size, so
    /// a site with many files never holds a request or a worker for long. [default: 500]
    #[arg(long, value_name = "N", env = "INK_REMOVE_STEP", help_heading = Heading::LIMITS)]
    pub remove_step: Option<u64>,
    /// Days a preview of a branch is served before it ends and is swept. Previews are public to
    /// anyone with the address. [default: 7]
    #[arg(long, value_name = "DAYS", env = "INK_PREVIEW_DAYS", help_heading = Heading::LIMITS)]
    pub preview_days: Option<u64>,
    /// The most custom domains one site has, proved and not. A claim of a name costs a row, so
    /// the number bounds what one site can ask for. [default: 10]
    #[arg(long, value_name = "N", env = "INK_LIMIT_DOMAINS", help_heading = Heading::LIMITS)]
    pub limit_domains: Option<u64>,
    /// Seconds one build may take: a worker holds a request for no longer than this, so a build
    /// that runs past it is given to another. [default: 600]
    #[arg(long, value_name = "SECONDS", env = "INK_LIMIT_BUILD_TIME", help_heading = Heading::LIMITS)]
    pub limit_build_time: Option<u64>,
}

impl Hosting {
    /// What the operator sets about the control plane: the flags over the defaults.
    pub(crate) fn policy(&self) -> Policy {
        let defaults = Limits::DEFAULT;
        let limits = Limits {
            stored: self.limit_stored.map_or(defaults.stored, Bytes),
            builds: self.limit_builds.unwrap_or(defaults.builds),
            sites: self.limit_sites.unwrap_or(defaults.sites),
            deploys: self.limit_deploys.unwrap_or(defaults.deploys),
            time: self.limit_build_time.map_or(defaults.time, Duration::from_secs),
        };
        let preview = self.preview_days.map_or(Policy::PREVIEW, |days| Duration::from_hours(days.saturating_mul(24)));
        let policy = Policy::DEFAULT
            .limited(limits)
            .stepping(self.remove_step.unwrap_or(Policy::STEP))
            .previewing(preview)
            .domained(self.limit_domains.unwrap_or(Policy::DOMAINS));
        self.handle_hold.map_or(policy, |days| policy.holding(Duration::from_hours(days.saturating_mul(24))))
    }

    /// The lookup's settings: the flags over the defaults.
    pub(crate) fn lookup(&self) -> edge_control::Settings {
        let defaults = edge_control::Settings::default();
        edge_control::Settings {
            ttl: Duration::from_secs(self.site_ttl.unwrap_or(0)),
            deploys: self.site_deploys.unwrap_or(defaults.deploys),
            ..defaults
        }
    }

    /// The kinds of git address allowed: the flags, or `https` alone.
    pub(crate) fn schemes(&self) -> Vec<Scheme> {
        if self.git_schemes.is_empty() {
            return vec![Scheme::Https];
        }
        let mut schemes: Vec<Scheme> = self.git_schemes.iter().copied().map(Scheme::from).collect();
        schemes.sort();
        schemes.dedup();
        schemes
    }

    /// What the builder may reach, from the schemes allowed.
    pub(crate) fn network(&self) -> Network {
        let schemes = self.schemes();
        if schemes.contains(&Scheme::Https) { Network::On } else { Network::Local }
    }

    /// When a crashed loop is started again.
    pub(crate) fn restart(&self) -> Restart {
        let defaults = Restart::default();
        Restart {
            limit: self.restart_limit.unwrap_or(defaults.limit),
            delay: self.restart_delay.map_or(defaults.delay, Duration::from_secs),
            healthy: self.restart_healthy.map_or(defaults.healthy, Duration::from_secs),
        }
    }

    /// How long a worker holds a request, and how long it rests when there is none. A request is
    /// never held longer than a build may take.
    pub(crate) fn worker(&self) -> (Duration, Duration) {
        let time = self.policy().limits.time;
        (
            self.worker_lease.map_or(time, Duration::from_secs).min(time),
            self.worker_idle.map_or(Duration::from_secs(2), Duration::from_secs),
        )
    }

    /// How long a fetch may take.
    pub(crate) fn fetch_time(&self) -> Duration {
        self.git_time.map_or(Duration::from_mins(2), Duration::from_secs)
    }

    /// How an https git remote is reached: the flags over the defaults.
    ///
    /// # Errors
    ///
    /// [`Misconfigured`] when a `--git-ca` file cannot be read.
    pub(crate) fn https(&self) -> Result<Https, Misconfigured> {
        let defaults = Https::default();
        let mut https = Https {
            connect: self.git_connect.map_or(defaults.connect, Duration::from_secs),
            total: self.git_request.map_or(Duration::from_mins(2), Duration::from_secs),
            follow: self.git_redirects.map_or(defaults.follow, Follow::from),
            ..defaults
        };
        for path in &self.git_cas {
            https = https.trusting_file(path)?;
        }
        Ok(https)
    }
}
