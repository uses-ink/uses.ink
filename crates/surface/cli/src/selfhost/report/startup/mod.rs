//! The start report of `ink serve`: one typed [`Startup`] value that is rendered twice, as the
//! sections a person reads (`render.rs`) and as the JSON document a script reads (`Serialize`), so
//! the two cannot drift (decision 0210).
//!
//! Every setting is a [`Row`] of a typed [`Value`] with its [`Source`]: the default, or the flag
//! that changed it. Nothing is printed that is not a row, and no row is only prose.

mod plan;
mod render;
mod row;
#[cfg(test)]
mod test;

use std::net::SocketAddr;
use std::path::PathBuf;

use serde::Serialize;

use crate::command::source::DEFAULT_OUT;
use crate::grammar::args::Serve;
use crate::selfhost::signin::Transport;

pub(crate) use plan::Plan;
pub(crate) use row::{Caution, Row, Source, Span, Value};

/// How far a serve got: it is running, or `--print-config` only worked out what it would run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    /// The listener is bound and the control plane is set up.
    Serving,
    /// Nothing was bound, made or written.
    Planned,
}

/// The first-run link, and what became of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum Admin {
    /// Made by this start. It is shown here once and kept nowhere.
    Issued {
        /// The link. A secret: opening it signs in as the owner.
        link: String,
        /// How long it works, in seconds in the JSON.
        valid: Span,
    },
    /// Not made: one was made before, or an account can already sign in.
    Kept,
    /// `--print-config` makes nothing, so there is no link.
    Unminted,
    /// There is no control plane to sign in to.
    Off,
}

/// The addresses a person opens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Urls {
    pub(crate) site: String,
    pub(crate) dashboard: Option<String>,
    pub(crate) api: Option<String>,
}

/// Which program says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Release {
    pub(crate) name: &'static str,
    pub(crate) version: &'static str,
}

impl Release {
    /// This build of `ink`.
    pub(crate) const fn this() -> Self {
        Self { name: "ink", version: env!("CARGO_PKG_VERSION") }
    }
}

/// A group of rows under a short heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Part {
    Addresses,
    SignIn,
    Data,
    Limits,
    Background,
    Security,
    Mail,
}

impl Part {
    /// The heading.
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Addresses => "Addresses",
            Self::SignIn => "Sign-in",
            Self::Data => "Data",
            Self::Limits => "Limits",
            Self::Background => "Background work",
            Self::Security => "Security",
            Self::Mail => "Mail",
        }
    }
}

/// The rows of one part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Section {
    pub(crate) part: Part,
    pub(crate) rows: Vec<Row>,
}

impl Section {
    fn new(part: Part) -> Self {
        Self { part, rows: Vec::new() }
    }

    #[must_use]
    fn row(mut self, row: Row) -> Self {
        self.rows.push(row);
        self
    }
}

/// What a control plane adds to the report: the resolved settings and what the start made.
#[derive(Clone, Debug)]
pub(crate) struct Hosted {
    pub(crate) plan: Plan,
    pub(crate) site: String,
    pub(crate) data: PathBuf,
    pub(crate) operators: PathBuf,
    /// The migrations this start applied, by name; none when nothing was opened.
    pub(crate) migrated: Option<Vec<String>>,
    pub(crate) admin: Admin,
    pub(crate) retired: bool,
}

/// What every serve knows: its flags, where it listens, and how far it got.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame<'a> {
    pub(crate) serve: &'a Serve,
    pub(crate) address: SocketAddr,
    pub(crate) mode: &'static str,
    pub(crate) stage: Stage,
}

/// Everything `ink serve` says when it starts, as data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Startup {
    schema: &'static str,
    address: String,
    mode: &'static str,
    stage: Stage,
    release: Release,
    urls: Urls,
    admin: Admin,
    sections: Vec<Section>,
}

impl Startup {
    /// The schema of the JSON form.
    pub(crate) const SCHEMA: &'static str = "uses.serve/v2";

    /// The report of a serve with `frame`, and a control plane when `--data` turned one on.
    pub(crate) fn new(frame: Frame<'_>, hosted: Option<&Hosted>) -> Self {
        let address = format!("http://{}", frame.address);
        let (urls, admin) = match hosted {
            Some(hosted) => {
                let api = crate::selfhost::signin::Flags::api_address(&hosted.site, &frame.serve.api_host);
                let urls = Urls {
                    site: hosted.site.clone(),
                    dashboard: Some(api.clone()),
                    api: Some(format!("{api}/v1/sites")),
                };
                (urls, hosted.admin.clone())
            }
            None => (Urls { site: address.clone(), dashboard: None, api: None }, Admin::Off),
        };
        let rows = Rows { frame, hosted };
        let mut sections = vec![rows.addresses()];
        if let Some(hosted) = hosted {
            sections.extend([
                rows.signin(hosted),
                Rows::data(hosted),
                rows.limits(hosted),
                rows.background(hosted),
                rows.security(hosted),
                rows.mail(hosted),
            ]);
        }
        Self {
            schema: Self::SCHEMA,
            address,
            mode: frame.mode,
            stage: frame.stage,
            release: Release::this(),
            urls,
            admin,
            sections,
        }
    }
}

/// The rows of each part, made from the flags and what was resolved.
struct Rows<'a> {
    frame: Frame<'a>,
    hosted: Option<&'a Hosted>,
}

impl Rows<'_> {
    fn addresses(&self) -> Section {
        let serve = self.frame.serve;
        let fallback = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
        let folder = serve.folder.as_deref().unwrap_or_else(|| std::path::Path::new(DEFAULT_OUT));
        let by_default = folder.ends_with(DEFAULT_OUT);
        let section = Section::new(Part::Addresses)
            .row(Row::new(
                "listen",
                Value::Text(self.frame.address.to_string()),
                Source::given(serve.address != fallback, "--addr"),
            ))
            .row(Row::new("folder", Value::path(folder), Source::given(!by_default, "FOLDER")))
            .row(Row::new(
                "host_mode",
                Value::Text(self.frame.mode.to_owned()),
                Source::changed(&serve.host_mode, "--host-mode"),
            ));
        let Some(hosted) = self.hosted else {
            return section.row(Row::new("api", Value::Text("off".to_owned()), Source::Needs("--data")));
        };
        let sites = hosted.site.trim_start_matches("http://").trim_start_matches("https://");
        let origin = if serve.origin.is_some() { Source::Flag("--origin") } else { Source::Derived("--origin") };
        section
            .row(Row::new("origin", Value::Url(hosted.site.clone()), origin))
            .row(Row::new(
                "api_host",
                Value::Text(serve.api_host.clone()),
                Source::given(serve.api_host != "api.localhost", "--api-host"),
            ))
            .row(Row::new("sites", Value::Text(format!("<handle>.{sites}")), Source::Computed))
    }

    #[expect(clippy::too_many_lines, reason = "one row for each setting")]
    fn signin(&self, hosted: &Hosted) -> Section {
        let (flags, plan) = (&self.frame.serve.signin, &hosted.plan);
        let section = Section::new(Part::SignIn)
            .row(Row::new(
                "login_link",
                Value::Seconds(Span(plan.sign.login.duration())),
                Source::given(flags.login_ttl.is_some(), "--login-ttl"),
            ))
            .row(Row::new(
                "session",
                Value::named(&flags.session_expiry),
                Source::changed(&flags.session_expiry, "--session-expiry"),
            ));
        let section = match plan.sign.session {
            control::signin::Expiry::Sliding { idle, limit } => section
                .row(Row::new(
                    "session_idle",
                    Value::Seconds(Span(idle.duration())),
                    Source::given(flags.session_idle.is_some(), "--session-idle"),
                ))
                .row(Row::new(
                    "session_limit",
                    Value::Seconds(Span(limit.duration())),
                    Source::given(flags.session_limit.is_some(), "--session-limit"),
                )),
            control::signin::Expiry::Absolute { ttl } => section.row(Row::new(
                "session_limit",
                Value::Seconds(Span(ttl.duration())),
                Source::given(flags.session_limit.is_some(), "--session-limit"),
            )),
            _ => section,
        };
        let secure = Row::new("cookie_secure", Value::Flag(plan.secure), Source::Derived("--origin"));
        let secure = if plan.secure { secure } else { secure.warned(Caution::PlainHttp) };
        let section = section
            .row(Row::new(
                "signup",
                Value::Text(plan.sign.signup.to_string()),
                Source::given(flags.signup.is_some(), "--signup"),
            ))
            .row(Row::new(
                "link_base",
                Value::Url(plan.base.clone()),
                Source::given(flags.login_base.is_some(), "--login-base"),
            ))
            .row(Row::new(
                "cookie",
                Value::Text(plan.cookie.clone()),
                Source::given(flags.cookie_name.is_some(), "--cookie-name"),
            ))
            .row(Row::new(
                "cookie_path",
                Value::Text(flags.cookie_path.clone().unwrap_or_else(|| "/".to_owned())),
                Source::given(flags.cookie_path.is_some(), "--cookie-path"),
            ))
            .row(secure)
            .row(Row::new(
                "passkey_id",
                Value::Text(plan.relying.clone()),
                Source::given(flags.rp_id.is_some(), "--rp-id"),
            ))
            .row(Row::new(
                "passkey_origins",
                Value::List(plan.origins.iter().cloned().map(Value::Url).collect()),
                Source::given(!flags.rp_origin.is_empty(), "--rp-origin"),
            ));
        match plan.forwarded {
            edge_axum::Forwarded::Hops(hops) => section
                .row(Row::new(
                    "client_address",
                    Value::Text("X-Forwarded-For".to_owned()),
                    Source::Flag("--forwarded-hops"),
                ))
                .row(Row::new(
                    "trusted_proxies",
                    Value::Count(u64::from(hops.get())),
                    Source::Flag("--forwarded-hops"),
                )),
            _ => section.row(Row::new("client_address", Value::Text("the connection".to_owned()), Source::Default)),
        }
    }

    fn data(hosted: &Hosted) -> Section {
        let section = Section::new(Part::Data)
            .row(Row::new("data", Value::path(&hosted.data), Source::Flag("--data")))
            .row(Row::new("database", Value::path(&hosted.data.join("ink.db")), Source::Computed))
            .row(Row::new("operator_keys", Value::path(&hosted.operators), Source::Computed))
            .row(Row::new(
                "enrol_yours",
                Value::Command(format!("ink login --data {}", hosted.data.display())),
                Source::Computed,
            ));
        let section = match &hosted.migrated {
            Some(names) if names.is_empty() => {
                section.row(Row::new("migrations", Value::Text("none was due".to_owned()), Source::Computed))
            }
            Some(names) => section.row(Row::new(
                "migrations",
                Value::List(names.iter().cloned().map(Value::Text).collect()),
                Source::Computed,
            )),
            None => section,
        };
        if hosted.retired {
            let file = hosted.data.join("owner-token");
            let caution = Caution::Retired { file: file.display().to_string() };
            return section.row(Row::new("owner_token", Value::path(&file), Source::Computed).warned(caution));
        }
        section
    }

    fn limits(&self, hosted: &Hosted) -> Section {
        let serve = self.frame.serve;
        let limits = &hosted.plan.limits;
        Section::new(Part::Limits)
            .row(Row::new(
                "json_body",
                Value::Bytes(limits.body),
                Source::given(serve.api_body.is_some(), "--api-body"),
            ))
            .row(Row::new(
                "upload_archive",
                Value::Bytes(limits.archive),
                Source::given(serve.upload_bytes.is_some(), "--upload-bytes"),
            ))
            .row(Row::new(
                "upload_files",
                Value::Count(limits.files),
                Source::given(serve.upload_files.is_some(), "--upload-files"),
            ))
            .row(Row::new(
                "upload_file",
                Value::Bytes(limits.file),
                Source::given(serve.upload_file.is_some(), "--upload-file"),
            ))
            .row(Row::new(
                "upload_tree",
                Value::Bytes(limits.tree),
                Source::given(serve.upload_tree.is_some(), "--upload-tree"),
            ))
            .row(Row::new(
                "upload_part",
                Value::Bytes(limits.part),
                Source::given(serve.upload_part.is_some(), "--upload-part"),
            ))
            .row(Row::new(
                "export_archive",
                Value::Bytes(limits.export),
                Source::given(serve.export_bytes.is_some(), "--export-bytes"),
            ))
            .row(Row::new("list_rows", Value::Count(u64::from(limits.rows)), Source::Default))
    }

    #[expect(clippy::too_many_lines, reason = "one row for each setting")]
    fn background(&self, hosted: &Hosted) -> Section {
        let hosting = &self.frame.serve.hosting;
        let serve = self.frame.serve;
        let (restart, (lease, rest)) = (hosting.restart(), hosting.worker());
        let interval = hosted.plan.interval;
        let schemes = hosting.schemes().iter().map(|scheme| Value::Text(scheme.to_string())).collect();
        Section::new(Part::Background)
            .row(Row::new(
                "poll_every",
                Value::Seconds(Span(interval.base())),
                Source::given(serve.poll_interval.is_some(), "--poll-interval"),
            ))
            .row(Row::new(
                "poll_backoff",
                Value::Seconds(Span(interval.max())),
                Source::given(serve.poll_max.is_some(), "--poll-max"),
            ))
            .row(Row::new(
                "new_site_within",
                Value::Seconds(Span(interval.lookout())),
                Source::given(hosting.poll_lookout.is_some(), "--poll-lookout"),
            ))
            .row(Row::new(
                "git_schemes",
                Value::List(schemes),
                Source::given(!hosting.git_schemes.is_empty(), "--git-scheme"),
            ))
            .row(Row::new(
                "preview_branches",
                Value::Count(u64::try_from(hosting.previews().0).unwrap_or(u64::MAX)),
                Source::given(hosting.preview_branches.is_some(), "--preview-branches"),
            ))
            .row(Row::new(
                "worker_lease",
                Value::Seconds(Span(lease)),
                Source::given(hosting.worker_lease.is_some(), "--worker-lease"),
            ))
            .row(Row::new(
                "worker_rest",
                Value::Seconds(Span(rest)),
                Source::given(hosting.worker_idle.is_some(), "--worker-idle"),
            ))
            .row(Row::new(
                "restart_after",
                Value::Seconds(Span(restart.delay)),
                Source::given(hosting.restart_delay.is_some(), "--restart-delay"),
            ))
            .row(Row::new(
                "restart_limit",
                Value::Count(u64::from(restart.limit)),
                Source::given(hosting.restart_limit.is_some(), "--restart-limit"),
            ))
    }

    #[expect(clippy::too_many_lines, reason = "one row for each setting")]
    fn security(&self, hosted: &Hosted) -> Section {
        let auth = &self.frame.serve.auth;
        let keys = &hosted.plan.keys;
        let span = |ttl: control::signin::Ttl| Value::Seconds(Span(ttl.duration()));
        Section::new(Part::Security)
            .row(Row::new(
                "audience",
                Value::Text(keys.audience.to_string()),
                if auth.audience.is_some() { Source::Flag("--audience") } else { Source::Derived("--audience") },
            ))
            .row(Row::new(
                "skew",
                Value::Seconds(Span(std::time::Duration::from_secs(keys.window.seconds()))),
                Source::given(auth.auth_skew.is_some(), "--auth-skew"),
            ))
            .row(Row::new(
                "nonces_per_key",
                Value::Count(u64::try_from(keys.nonces).unwrap_or(u64::MAX)),
                Source::given(auth.auth_nonces.is_some(), "--auth-nonces"),
            ))
            .row(Row::new(
                "device_keys",
                span(keys.keys.device),
                Source::given(auth.key_device.is_some(), "--key-device"),
            ))
            .row(Row::new(
                "deploy_keys",
                span(keys.keys.deploy),
                Source::given(auth.key_deploy.is_some(), "--key-deploy"),
            ))
            .row(Row::new(
                "key_ceiling",
                span(keys.keys.ceiling),
                Source::given(auth.key_ceiling.is_some(), "--key-ceiling"),
            ))
            .row(Row::new(
                "enrol_approval",
                span(keys.keys.enrol),
                Source::given(auth.enrol_ttl.is_some(), "--enrol-ttl"),
            ))
            .row(Row::new(
                "enrol_guesses",
                Value::Count(u64::from(keys.closing.guesses)),
                Source::given(auth.enrol_guesses.is_some(), "--enrol-guesses"),
            ))
            .row(Row::new(
                "enrol_waiting",
                Value::Count(keys.closing.waiting),
                Source::given(auth.enrol_waiting.is_some(), "--enrol-waiting"),
            ))
            .row(Row::new(
                "enrol_closed",
                span(keys.closing.closed),
                Source::given(auth.enrol_closed.is_some(), "--enrol-closed"),
            ))
    }

    #[expect(clippy::too_many_lines, reason = "one row for each setting")]
    fn mail(&self, hosted: &Hosted) -> Section {
        let (flags, plan) = (&self.frame.serve.signin, &hosted.plan);
        let section = Section::new(Part::Mail).row(Row::new(
            "transport",
            Value::named(&flags.mail),
            Source::changed(&flags.mail, "--mail"),
        ));
        let section = match flags.mail {
            Transport::Stdout => section.row(Row::new(
                "messages",
                Value::Text("printed here, links included".to_owned()),
                Source::Computed,
            )),
            Transport::Smtp => {
                let port = flags.smtp_port.map_or(0, u64::from);
                section
                    .row(Row::new(
                        "smtp_host",
                        Value::Text(flags.smtp_host.clone().unwrap_or_default()),
                        Source::Flag("--smtp-host"),
                    ))
                    .row(Row::new(
                        "smtp_port",
                        Value::Count(port),
                        Source::given(flags.smtp_port.is_some(), "--smtp-port"),
                    ))
                    .row(Row::new(
                        "smtp_security",
                        Value::named(&flags.smtp_security),
                        Source::changed(&flags.smtp_security, "--smtp-security"),
                    ))
                    .row(Row::new(
                        "mail_from",
                        Value::Text(flags.mail_from.clone().unwrap_or_default()),
                        Source::Flag("--mail-from"),
                    ))
                    .row(Row::new(
                        "smtp_password",
                        if flags.smtp_user.is_some() { Value::Secret } else { Value::Text("none".to_owned()) },
                        Source::given(flags.smtp_user.is_some(), "--smtp-user"),
                    ))
            }
        };
        let rates = plan.rates;
        let rate = |rate: edge_api::Rate| Value::Rate { count: rate.count, per: Span(rate.window.duration()) };
        section
            .row(Row::new(
                "ask_per_address",
                rate(rates.address),
                Source::given(flags.rate_address.is_some(), "--rate-address"),
            ))
            .row(Row::new(
                "ask_per_source",
                rate(rates.source),
                Source::given(flags.rate_source.is_some(), "--rate-source"),
            ))
            .row(Row::new(
                "asks_tracked",
                Value::Count(u64::try_from(rates.tracked).unwrap_or(u64::MAX)),
                Source::given(flags.rate_tracked.is_some(), "--rate-tracked"),
            ))
            .row(Row::new(
                "outbox",
                Value::Count(u64::try_from(plan.outbox).unwrap_or(u64::MAX)),
                Source::given(flags.outbox.is_some(), "--outbox"),
            ))
    }
}
