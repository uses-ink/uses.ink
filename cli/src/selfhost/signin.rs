//! The sign-in settings of `ink serve --data`: flags and environment variables, and the adapters
//! they choose (decision 0066). `args::Serve` flattens [`Flags`]; `product` builds the door
//! from it.

use std::fmt;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

use clap::{Args, ValueEnum};
use control::signin::{Expiry, Settings, Signup, Ttl};
use edge_api::{Note, Notes, Rate, Rates};
use mail::{Address, LinkBase, Mailer, Message};
use mail_smtp::{Login, Security, Settings as Smtp, Smtp as Mail};
use secret::Plain;

use crate::grammar::tree::Heading;
use crate::selfhost::product::Unready;

/// Where mail goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Transport {
    /// Written to this terminal's standard output, links included: for a first run and for
    /// development, with no mail server.
    #[default]
    Stdout,
    /// Sent to an SMTP server.
    Smtp,
}

/// How the connection to the SMTP server is secured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Secured {
    /// `STARTTLS`, which the server must offer (port 587).
    #[default]
    Starttls,
    /// TLS from the first byte (port 465).
    Implicit,
    /// No encryption: a server on this machine, or a test.
    Plain,
}

/// How a session ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Lifetime {
    /// Ends after `--session-idle` unused, and `--session-limit` at most.
    #[default]
    Sliding,
    /// Ends `--session-limit` after it began, however it is used.
    Absolute,
}

/// The flags of signing in. Each has an environment variable (`INK_` and the flag's name) and a
/// default that `ink serve --data` prints when it starts.
#[derive(Debug, Args)]
pub struct Flags {
    /// Where sign-in mail goes.
    #[arg(long, value_enum, env = "INK_MAIL", default_value_t, help_heading = Heading::MAIL)]
    pub mail: Transport,
    /// The address mail is sent from. Required with `--mail smtp`.
    #[arg(long, env = "INK_MAIL_FROM", help_heading = Heading::MAIL)]
    pub mail_from: Option<String>,
    /// The SMTP server's host name.
    #[arg(long, env = "INK_SMTP_HOST", help_heading = Heading::MAIL)]
    pub smtp_host: Option<String>,
    /// The SMTP server's port. [default: 587, or 465 with `--smtp-security implicit`]
    #[arg(long, env = "INK_SMTP_PORT", help_heading = Heading::MAIL)]
    pub smtp_port: Option<u16>,
    /// How the SMTP connection is secured.
    #[arg(long, value_enum, env = "INK_SMTP_SECURITY", default_value_t, help_heading = Heading::MAIL)]
    pub smtp_security: Secured,
    /// The SMTP username.
    #[arg(long, env = "INK_SMTP_USER", help_heading = Heading::MAIL)]
    pub smtp_user: Option<String>,
    /// A file holding the SMTP password. `INK_SMTP_PASSWORD` holds it instead; there is no
    /// flag for the password itself, which a process list would show.
    #[arg(long, env = "INK_SMTP_PASSWORD_FILE", help_heading = Heading::MAIL)]
    pub smtp_password_file: Option<PathBuf>,
    /// Where the link in a message points; the token is added as one more path segment.
    /// [default: the API's address, `/v1/login`]
    #[arg(long, env = "INK_LOGIN_BASE", help_heading = Heading::MAIL)]
    pub login_base: Option<String>,
    /// Whether opening the link with a GET signs in, which a mail scanner can do for you.
    /// Without it a GET only looks, and a page in front of the API POSTs.
    #[arg(long, env = "INK_LOGIN_GET_SIGNS_IN", help_heading = Heading::SIGN_IN)]
    pub login_get_signs_in: bool,
    /// How long a sign-in link works. [default: 15m]
    #[arg(long, env = "INK_LOGIN_TTL", help_heading = Heading::SIGN_IN)]
    pub login_ttl: Option<Ttl>,
    /// How long the first-run link works. [default: 24h]
    #[arg(long, env = "INK_ADMIN_TTL", help_heading = Heading::SIGN_IN)]
    pub admin_ttl: Option<Ttl>,
    /// How a session ends.
    #[arg(long, value_enum, env = "INK_SESSION_EXPIRY", default_value_t, help_heading = Heading::SIGN_IN)]
    pub session_expiry: Lifetime,
    /// How long a sliding session may sit unused. [default: 7d]
    #[arg(long, env = "INK_SESSION_IDLE", help_heading = Heading::SIGN_IN)]
    pub session_idle: Option<Ttl>,
    /// The longest a session lasts. [default: 30d]
    #[arg(long, env = "INK_SESSION_LIMIT", help_heading = Heading::SIGN_IN)]
    pub session_limit: Option<Ttl>,
    /// Who may make an account by signing in: `first` (the first, by the first-run link),
    /// `open` (any address that receives mail) or `closed`. [default: first]
    #[arg(long, env = "INK_SIGNUP", help_heading = Heading::SIGN_IN)]
    pub signup: Option<Signup>,
    /// Requests for a link per address, as `<count>/<window>`. [default: 5/1h]
    #[arg(long, env = "INK_RATE_ADDRESS", help_heading = Heading::SIGN_IN)]
    pub rate_address: Option<Rate>,
    /// Requests for a link per source (an IPv4 address or an IPv6 /64). [default: 20/1h]
    #[arg(long, env = "INK_RATE_SOURCE", help_heading = Heading::SIGN_IN)]
    pub rate_source: Option<Rate>,
    /// How many addresses and sources the limits remember at once. [default: 10000]
    #[arg(long, env = "INK_RATE_TRACKED", help_heading = Heading::SIGN_IN)]
    pub rate_tracked: Option<usize>,
    /// How many messages wait to be sent before more are dropped. [default: 256]
    #[arg(long, env = "INK_OUTBOX", help_heading = Heading::MAIL)]
    pub outbox: Option<usize>,
    /// How long the mail sender rests when nothing waits. [default: 1s]
    #[arg(long, env = "INK_COURIER_REST", help_heading = Heading::MAIL)]
    pub courier_rest: Option<Ttl>,
    /// The session cookie's name. [default: `__Host-ink-session`, or `ink-session` on plain http]
    #[arg(long, env = "INK_COOKIE_NAME", help_heading = Heading::SIGN_IN)]
    pub cookie_name: Option<edge_api::Name>,
    /// The session cookie's path. [default: /]
    #[arg(long, env = "INK_COOKIE_PATH", help_heading = Heading::SIGN_IN)]
    pub cookie_path: Option<String>,
    /// The relying party id of passkeys: a host name the dashboard is served on or above. A
    /// passkey made under one id never works under another. [default: the API host]
    #[arg(long, env = "INK_RP_ID", help_heading = Heading::SIGN_IN)]
    pub rp_id: Option<String>,
    /// An origin the dashboard is served from, which passkey ceremonies must come from. Repeat
    /// for more than one. [default: the origin of the API host]
    #[arg(long, env = "INK_RP_ORIGIN", value_delimiter = ',', help_heading = Heading::SIGN_IN)]
    pub rp_origin: Vec<String>,
    /// An origin besides the API's own that may post state-changing requests with a session
    /// cookie, such as a dashboard served on another host. Repeat for more than one.
    #[arg(long, env = "INK_ALLOW_ORIGIN", value_delimiter = ',', help_heading = Heading::SIGN_IN)]
    pub allow_origin: Vec<String>,
    /// Passkey sign-ins that may start per source, as `<count>/<window>`. [default: 60/1h]
    #[arg(long, env = "INK_CHALLENGE_LIMIT", help_heading = Heading::SIGN_IN)]
    pub challenge_limit: Option<Rate>,
    /// Recovery codes that may be tried per source, as `<count>/<window>`. [default: 10/1h]
    #[arg(long, env = "INK_RECOVERY_LIMIT", help_heading = Heading::SIGN_IN)]
    pub recovery_limit: Option<Rate>,
    /// How recent a passkey sign-in must be to add or remove a passkey, approve a key or make
    /// recovery codes. [default: 5m]
    #[arg(long, env = "INK_STEP_UP_WINDOW", help_heading = Heading::SIGN_IN)]
    pub step_up_window: Option<Ttl>,
    /// How many reverse proxies of yours sit in front of the server. With it, the client's address
    /// is read from `X-Forwarded-For`, that many entries from the end, and a request without it is
    /// refused. [default: off, the connection's address is used]
    #[arg(long, env = "INK_FORWARDED_HOPS", help_heading = Heading::SIGN_IN)]
    pub forwarded_hops: Option<std::num::NonZeroU8>,
}

/// An origin that is plain `http` on a name that is not this machine's: a session cookie would
/// cross the network in the clear.
#[derive(Debug)]
pub(crate) struct Exposed {
    /// The origin.
    pub origin: String,
}

impl fmt::Display for Exposed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} is plain http on a name that is not local: serve it over https", self.origin)
    }
}

impl std::error::Error for Exposed {}

/// A setting that needs another one with it.
#[derive(Debug)]
pub(crate) struct Missing {
    /// What is missing.
    pub flag: &'static str,
    /// What needs it.
    pub needed_by: &'static str,
}

impl fmt::Display for Missing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} needs {}", self.needed_by, self.flag)
    }
}

impl std::error::Error for Missing {}

impl Flags {
    /// How many messages the outbox holds when `--outbox` is not given.
    pub(crate) const OUTBOX: usize = 256;
    /// How long the sender rests when `--courier-rest` is not given.
    pub(crate) const REST: Ttl = Ttl::secs(1);

    fn setting<E: std::error::Error + Send + Sync + 'static>(flag: &'static str) -> impl FnOnce(E) -> Unready {
        move |error| Unready::Setting { flag, source: Box::new(error) }
    }

    /// The settings of the control plane's sign-in: the flags over the defaults.
    pub(crate) fn settings(&self) -> Settings {
        let defaults = Settings::default();
        let limit = self.session_limit.unwrap_or(Settings::LIMIT);
        let session = match self.session_expiry {
            Lifetime::Sliding => {
                Expiry::Sliding { idle: self.session_idle.unwrap_or(Settings::IDLE).min(limit), limit }
            }
            Lifetime::Absolute => Expiry::Absolute { ttl: limit },
        };
        Settings {
            login: self.login_ttl.unwrap_or(defaults.login),
            admin: self.admin_ttl.unwrap_or(defaults.admin),
            session,
            signup: self.signup.unwrap_or(defaults.signup),
        }
    }

    /// How the client's address is found: the connection's, or behind this many proxies.
    pub(crate) fn forwarded(&self) -> edge_axum::Forwarded {
        self.forwarded_hops.map_or(edge_axum::Forwarded::Off, edge_axum::Forwarded::Hops)
    }

    /// The limits on asking for links.
    pub(crate) fn rates(&self) -> Rates {
        let defaults = Rates::default();
        Rates {
            address: self.rate_address.unwrap_or(defaults.address),
            source: self.rate_source.unwrap_or(defaults.source),
            tracked: self.rate_tracked.unwrap_or(defaults.tracked),
        }
    }

    /// Whether the instance is served over TLS: `https`, or plain `http` on this machine.
    pub(crate) fn secure(origin: &str) -> Result<bool, Unready> {
        if origin.starts_with("https://") {
            return Ok(true);
        }
        let host = origin.trim_start_matches("http://").split(['/', ':']).next().unwrap_or_default();
        let local = host == "localhost" || host.ends_with(".localhost") || host == "127.0.0.1" || host == "[::1]";
        if local { Ok(false) } else { Err(Self::setting("--origin")(Exposed { origin: origin.to_owned() })) }
    }

    /// Who passkeys are made for: the flags, or the API host on the scheme and port of `origin`.
    pub(crate) fn relying(&self, origin: &str, api_host: &str) -> Result<control::passkeys::Relying, Unready> {
        let base = Self::api_base(origin, api_host);
        let site = base.trim_end_matches("/v1/login").to_owned();
        let host = api_host.rsplit_once(':').map_or(api_host, |(host, _)| host).to_ascii_lowercase();
        let id = self.rp_id.clone().unwrap_or(host);
        let origins = if self.rp_origin.is_empty() { vec![site] } else { self.rp_origin.clone() };
        let origins: Vec<&str> = origins.iter().map(String::as_str).collect();
        control::passkeys::Relying::new(&id, "uses.ink", &origins).map_err(Self::setting("--rp-id"))
    }

    /// The ceremony cookie: beside the session cookie, with its own name.
    pub(crate) fn ceremony(secure: bool) -> Result<edge_api::Jar, Unready> {
        let name = if secure { "__Host-ink-ceremony" } else { "ink-ceremony" };
        let name = name.parse::<edge_api::Name>().map_err(Self::setting("--cookie-name"))?;
        edge_api::Jar::new(Some(name), "/", secure).map_err(Self::setting("--cookie-name"))
    }

    /// The cookie.
    pub(crate) fn jar(&self, secure: bool) -> Result<edge_api::Jar, Unready> {
        let path = self.cookie_path.as_deref().unwrap_or("/");
        edge_api::Jar::new(self.cookie_name.clone(), path, secure).map_err(Self::setting("--cookie-name"))
    }

    /// The API's own login address on the origin's scheme and port.
    fn api_base(origin: &str, api_host: &str) -> String {
        let (scheme, rest) = origin.split_once("://").unwrap_or(("http", origin));
        let authority = rest.split('/').next().unwrap_or_default();
        let port = authority.rsplit_once(':').map_or_else(String::new, |(_, port)| format!(":{port}"));
        format!("{scheme}://{api_host}{port}/v1/login")
    }

    /// Where links point: the flag, or the API's own `/v1/login`.
    pub(crate) fn base(&self, origin: &str, api_host: &str) -> Result<LinkBase, Unready> {
        let text = self.login_base.clone().unwrap_or_else(|| Self::api_base(origin, api_host));
        text.parse().map_err(Self::setting("--login-base"))
    }

    /// The mail adapter the flags choose.
    pub(crate) fn sender(&self) -> Result<Sender, Unready> {
        match self.mail {
            Transport::Stdout => Ok(Sender::Printed(mail::Stdout::new(std::io::stdout()))),
            Transport::Smtp => {
                let need = |flag| Self::setting("--mail")(Missing { flag, needed_by: "--mail smtp" });
                let host = self.smtp_host.clone().ok_or_else(|| need("--smtp-host"))?;
                let from: Address = self
                    .mail_from
                    .as_deref()
                    .ok_or_else(|| need("--mail-from"))?
                    .parse()
                    .map_err(Self::setting("--mail-from"))?;
                let port = self.smtp_port.unwrap_or(if self.smtp_security == Secured::Implicit { 465 } else { 587 });
                let login = self.login()?;
                let security = match self.smtp_security {
                    Secured::Starttls => Security::Starttls,
                    Secured::Implicit => Security::Implicit,
                    Secured::Plain => Security::Plain,
                };
                let settings = Smtp { host, port, security, login, from };
                Mail::new(settings).map(|mailer| Sender::Smtp(Box::new(mailer))).map_err(Self::setting("--smtp-host"))
            }
        }
    }

    /// The SMTP login: the user and the password from the environment or a file.
    fn login(&self) -> Result<Option<Login>, Unready> {
        let Some(username) = self.smtp_user.clone() else { return Ok(None) };
        let password = match (&self.smtp_password_file, std::env::var("INK_SMTP_PASSWORD")) {
            (Some(path), _) => std::fs::read_to_string(path)
                .map_err(|source| Unready::Data { path: path.clone(), source })?
                .trim_end_matches(['\n', '\r'])
                .to_owned(),
            (None, Ok(password)) => password,
            (None, Err(_)) => {
                return Err(Self::setting("--smtp-user")(Missing {
                    flag: "INK_SMTP_PASSWORD or --smtp-password-file",
                    needed_by: "--smtp-user",
                }));
            }
        };
        Ok(Some(Login { username, password: Plain::from(password.as_str()) }))
    }
}

/// The mail adapter in use.
#[derive(Debug)]
pub(crate) enum Sender {
    /// Messages written to standard output.
    Printed(mail::Stdout<std::io::Stdout>),
    /// Messages sent over SMTP.
    Smtp(Box<Mail>),
}

/// Why a message was not sent.
#[derive(Debug)]
pub(crate) enum Unsent {
    /// Standard output could not be written.
    Print(std::io::Error),
    /// The SMTP server could not be reached or refused it.
    Smtp(mail_smtp::Error),
}

impl fmt::Display for Unsent {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Print(_) => out.write_str("the message could not be written to standard output"),
            Self::Smtp(_) => out.write_str("the message could not be sent over SMTP"),
        }
    }
}

impl std::error::Error for Unsent {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Print(source) => Some(source),
            Self::Smtp(source) => Some(source),
        }
    }
}

impl Mailer for Sender {
    type Error = Unsent;

    async fn send(&self, message: &Message) -> Result<(), Unsent> {
        match self {
            Self::Printed(mailer) => mailer.send(message).await.map_err(Unsent::Print),
            Self::Smtp(mailer) => mailer.send(message).await.map_err(Unsent::Smtp),
        }
    }
}

/// What the mail sender says, as lines on standard error each after the time it happened, like
/// the server's own events.
#[derive(Debug)]
pub(crate) struct Told(pub(crate) Mutex<std::io::Stderr>);

impl Notes for Told {
    fn note(&self, note: &Note) {
        let mut stderr = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // a closed stderr leaves nowhere to report to
        writeln!(stderr, "{} mail: {note}", jiff::Timestamp::now()).ok();
        if let Note::Failed { source } = note {
            let mut cause: Option<&dyn std::error::Error> = Some(source.as_ref());
            while let Some(error) = cause {
                writeln!(stderr, "  caused by: {error}").ok();
                cause = error.source();
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_plain_http_origin_must_be_local_and_https_is_secure() {
        assert!(Flags::secure("https://ink.example").unwrap());
        assert!(!Flags::secure("http://localhost:8080").unwrap());
        assert!(!Flags::secure("http://site.localhost").unwrap());
        assert!(Flags::secure("http://ink.example").is_err());
    }

    #[test]
    fn the_default_link_base_is_the_apis_login_on_the_origins_port() {
        assert_eq!(Flags::api_base("http://localhost:8080", "api.localhost"), "http://api.localhost:8080/v1/login");
        assert_eq!(Flags::api_base("https://ink.example", "api.ink.example"), "https://api.ink.example/v1/login");
    }
}
