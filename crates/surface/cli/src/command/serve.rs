//! `ink serve [folder]`: the deploy `ink build` wrote, served by the edge crate.
//!
//! `ink preview` builds the site in memory and serves it for writing. This serves what was
//! already built, through the same `edge` code the hosted product runs: its refusals, budgets,
//! conditional and range handling and the manifest's headers, unchanged.

use std::io::Write;
use std::sync::{Arc, Mutex};

use edge::{Edge, Limits, Scheme};
use edge_axum::{Bridge, Event, Local, Mount, Observe, Server};
use edge_control::{Fallback, Fronts};
use edge_folder::{Deploys, Files, Mode};
use serde::Serialize;

use crate::command::remote::{Fetching, Settings};
use crate::command::source::DEFAULT_OUT;
use crate::grammar::args::{self, HostMode};
use crate::output::error::{Error, ServeError};
use crate::output::exit::Exit;
use crate::output::term::{Answer, Line, Record, Role};
use crate::output::{Io, Say};
use crate::selfhost::product::{Mailing, Product};
use crate::selfhost::signin::{Sender, Told, Transport};
use crate::selfhost::tasks::{Event as Tasked, Events, Stoppable, Task, Tasks};

impl From<HostMode> for Mode {
    fn from(mode: HostMode) -> Self {
        match mode {
            HostMode::Auto => Self::Auto,
            HostMode::Single => Self::Single,
            HostMode::Folders => Self::Folders,
        }
    }
}

/// What `ink serve --json` prints before it serves: where, and how hosts map to folders.
#[derive(Debug, Serialize)]
struct Started {
    schema: &'static str,
    address: String,
    mode: &'static str,
}

/// Events as lines on stderr, each after the time it happened: the one-shot commands log at warn
/// without timestamps, `ink serve` logs at info with them (conventions.md, CLI output).
#[derive(Debug)]
struct Log(Mutex<std::io::Stderr>);

impl Log {
    /// One line of what a loop did, after the time it did it.
    fn tasked(&self, event: &Tasked) {
        let mut stderr = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // a closed stderr leaves nowhere to report to
        writeln!(stderr, "{} {event}", jiff::Timestamp::now()).ok();
    }
}

impl Observe for Log {
    fn event(&self, event: &Event) {
        let mut stderr = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // a closed stderr leaves nowhere to report to
        writeln!(stderr, "{} {event}", jiff::Timestamp::now()).ok();
    }
}

/// What runs beside the server when there is an API: the half that answers it, and the mail
/// sender with where it reports.
type Beside = (Local<crate::selfhost::product::Wired>, edge_api::Courier<Sender, clock::System>, Told);

/// The sites a request's host picks from: the control plane's, then the folder's.
type Sites = Fallback<Fronts, Deploys>;

/// The server, and what runs beside it when `--data` turned the API on.
type Attached = (Server<Sites, Files, Log>, Option<Beside>);

/// What runs beside the server and the API: the edge's lookups and the loops' events.
struct Alongside {
    back: Option<crate::selfhost::product::Lookup>,
    told: tokio::sync::mpsc::UnboundedReceiver<Tasked>,
}

impl args::Serve {
    /// The server with the API mounted, when `--data` turned it on, and the half that answers it.
    fn attach(
        &self,
        server: Server<Sites, Files, Log>,
        product: Option<Product>,
        io: &mut Io<'_>,
    ) -> Result<Attached, Error> {
        let (Some(data), Some(product)) = (&self.data, product) else {
            let off = Line::marked(Role::Plain, "off; give `--data <folder>` to turn on the control plane and its API");
            io.aside(Say::Warnings, &Record::new(0).field("api", off))?;
            return Ok((server, None));
        };
        let (mount, local) = self.start_api(product, data, io)?;
        Ok((server.with_api(mount), Some(local)))
    }

    /// Sets the control plane up, prints every setting it runs with, and returns the API's way
    /// into the server and the half that answers.
    #[expect(clippy::too_many_lines, reason = "one listing of every setting, a field each")]
    fn start_api(&self, product: Product, data: &std::path::Path, io: &mut Io<'_>) -> Result<(Mount, Beside), Error> {
        let (limits, interval) = (product.limits, product.interval);
        let sites = product.site.trim_start_matches("http://").trim_start_matches("https://");
        let record = Record::new(0)
            .field(
                "api",
                Line::plain(format!(
                    "{}/v1/sites (host {})",
                    crate::selfhost::signin::Flags::api_address(&product.site, &self.api_host),
                    self.api_host
                )),
            )
            .field(
                "origin",
                Line::plain(if self.origin.is_some() {
                    format!("{} (from --origin)", product.site)
                } else {
                    format!("{} (from the listen address; give --origin to change it)", product.site)
                }),
            )
            .field(
                "dashboard",
                Line::plain(format!(
                    "{} (sign in with the link `ink` prints or mails)",
                    crate::selfhost::signin::Flags::api_address(&product.site, &self.api_host)
                )),
            )
            .field("data", Line::plain(data.display().to_string()))
            .field(
                "operator keys",
                Line::plain(format!(
                    "{} (enrol yours with `ink login --data {}`)",
                    product.operators.display(),
                    data.display()
                )),
            )
            .field("signatures", Line::plain(Self::describe_keys(&product.keys)))
            .field("sites", Line::plain(format!("<handle>.{sites}")))
            .field(
                "limits",
                Line::plain(format!(
                    "json body {} B, upload {} B, {} files, {} B a file, export {} B, lists {} rows",
                    limits.body, limits.archive, limits.files, limits.file, limits.export, limits.rows
                )),
            );
        let record = match &product.retired {
            Some(file) => {
                let note = model::Announcement::AuthTokenRetired { token_file: file.display().to_string() };
                record.field(
                    "owner token",
                    Line::plain(format!(
                        "{} ({})",
                        diag::Message::from(&note),
                        diag::Fix::of_announcement(&note).map_or_else(String::new, |fix| fix.to_string())
                    )),
                )
            }
            None => record,
        };
        let record = self.describe_loops(interval, record);
        let record = self.list_signin(&product.mailing, record)?;
        io.aside(Say::Warnings, &record)?;
        let (bridge, local) = Bridge::new(product.api, limits);
        let Mailing { courier, told, forwarded, .. } = product.mailing;
        let bridge = bridge.forwarded(forwarded);
        Ok((Mount { host: self.api_host.to_ascii_lowercase().into(), bridge }, (local, courier, told)))
    }

    /// Says how requests are verified and keys are enrolled.
    fn describe_keys(keys: &crate::command::auth::Resolved) -> String {
        format!(
            "audience {}, skew {}s, {} nonces a key; keys live {} (device), {} (deploy), at most {}; a person has {} to approve; {} wrong codes or {} waiting close enrolling for {}",
            keys.audience,
            keys.window.seconds(),
            keys.nonces,
            keys.keys.device,
            keys.keys.deploy,
            keys.keys.ceiling,
            keys.keys.enrol,
            keys.closing.guesses,
            keys.closing.waiting,
            keys.closing.closed,
        )
    }

    /// Says what the watch and the worker run with.
    fn describe_loops(&self, interval: watch::Interval, record: Record) -> Record {
        let schemes: Vec<String> = self.hosting.schemes().iter().map(ToString::to_string).collect();
        let (restart, (lease, idle)) = (self.hosting.restart(), self.hosting.worker());
        let poll = format!(
            "every {}s, backing off to {}s, a new site within {}s; git over {}",
            interval.base().as_secs(),
            interval.max().as_secs(),
            interval.lookout().as_secs(),
            schemes.join(" and ")
        );
        let worker = format!(
            "lease {}s, rests {}s; a loop that crashes restarts after {}s, {} times in a row",
            lease.as_secs(),
            idle.as_secs(),
            restart.delay.as_secs(),
            restart.limit
        );
        record.field("poll", Line::plain(poll)).field("worker", Line::plain(worker))
    }

    /// Starts the watch and the worker over `product`'s control plane.
    fn start_tasks(&self, product: &Product, events: &Events) -> Tasks {
        let mut tasks = Tasks::new();
        self.start_watch(&mut tasks, product, events);
        self.start_worker(&mut tasks, product, events);
        tasks
    }

    fn start_watch(&self, tasks: &mut Tasks, product: &Product, events: &Events) {
        let (ports, origin, interval) = (Arc::clone(&product.ports), product.origin.clone(), product.interval);
        let policy = self.hosting.policy();
        let git = self.hosting.git_cache.clone().unwrap_or_else(|| product.data.join("git"));
        let (heads, network, https) = (git.join("heads"), self.hosting.network(), product.https.clone());
        tasks.start(Task::Watch, (self.hosting.restart(), events), move |clock: Stoppable, events: Events| {
            let (ports, origin) = (Arc::clone(&ports), origin.clone());
            let heads = watch_git::Remotes::new(heads.clone(), network).with_https(https.clone());
            Box::pin(async move {
                let control = control::Control::new(ports, origin).with_policy(policy);
                let mut watch = watch::Watch::new(&control, heads, (clock, clock::System), interval);
                watch.run(&events).await;
            })
        });
    }

    fn start_worker(&self, tasks: &mut Tasks, product: &Product, events: &Events) {
        let (ports, origin) = (Arc::clone(&product.ports), product.origin.clone());
        let policy = self.hosting.policy();
        let git = self.hosting.git_cache.clone().unwrap_or_else(|| product.data.join("git"));
        let settings = Settings {
            cache: git.join("clones"),
            components: product.data.join("components"),
            network: self.hosting.network(),
            https: product.https.clone(),
            budget: host_wasmtime::Budget { time: self.hosting.fetch_time(), ..host_wasmtime::Budget::default() },
        };
        let (blobs, times) = (product.data.join("blobs"), self.hosting.worker());
        tasks.start(Task::Worker, (self.hosting.restart(), events), move |clock: Stoppable, events: Events| {
            let (ports, origin, settings, blobs) =
                (Arc::clone(&ports), origin.clone(), settings.clone(), blobs.clone());
            let remote =
                Fetching::new(control::Control::new(Arc::clone(&ports), origin.clone()).with_policy(policy), settings);
            Box::pin(async move {
                let stored = match blob_dir::Directory::open(&blobs) {
                    Ok(stored) => stored,
                    Err(error) => {
                        events.tell(Tasked::Failed { cause: edge_control::Chain::of(&error) });
                        return;
                    }
                };
                let control = control::Control::new(ports, origin).with_policy(policy);
                let builder = build_process::Local::new(stored).with_remote(remote);
                watch::Worker::new(&control, builder, clock, times).run(&events).await;
            })
        });
    }

    /// Says where mail goes.
    fn list_mail(&self, record: Record) -> Record {
        let flags = &self.signin;
        let said = match flags.mail {
            Transport::Stdout => "stdout (messages and their links are printed here)".to_owned(),
            Transport::Smtp => format!(
                "smtp {}:{} {:?} from {} (password {})",
                flags.smtp_host.as_deref().unwrap_or_default(),
                flags.smtp_port.unwrap_or(0),
                flags.smtp_security,
                flags.mail_from.as_deref().unwrap_or_default(),
                if flags.smtp_user.is_some() { "set, never printed" } else { "none" },
            ),
        };
        record.field("mail", Line::plain(said))
    }

    /// Says every sign-in setting it runs with, and the first-run link when this start made it.
    /// Says who passkeys are made for.
    fn list_passkeys(relying: &control::passkeys::Relying, record: Record) -> Record {
        record.field(
            "passkeys",
            Line::plain(format!("relying party id {}, origins {}", relying.id(), relying.origins().join(", "))),
        )
    }

    /// Says where the client's address is read from.
    fn list_client(forwarded: edge_axum::Forwarded, record: Record) -> Record {
        record.field(
            "client address",
            Line::plain(match forwarded {
                edge_axum::Forwarded::Hops(hops) => format!("X-Forwarded-For, {hops} trusted proxies"),
                _ => "the connection's address (--forwarded-hops is off)".to_owned(),
            }),
        )
    }

    fn list_signin(&self, mailing: &Mailing, record: Record) -> Result<Record, Error> {
        let flags = &self.signin;
        let settings = &mailing.settings;
        let record = record.field(
            "sign-in",
            Line::plain(format!(
                "link {}, session {}, sign-up {}, link base {}",
                settings.login, settings.session, settings.signup, mailing.base
            )),
        );
        let record = self.list_mail(record).field(
            "ask limits",
            Line::plain(format!(
                "{} per address, {} per source, {} tracked, outbox {}",
                mailing.rates.address, mailing.rates.source, mailing.rates.tracked, mailing.outbox
            )),
        );
        let jar = flags.jar(mailing.secure).map_err(|source| ServeError::Product { source })?;
        let record = record.field(
            "cookie",
            Line::plain(format!(
                "{} path {} ({})",
                jar.name(),
                flags.cookie_path.as_deref().unwrap_or("/"),
                if mailing.secure { "Secure" } else { "plain http, not Secure" }
            )),
        );
        let record = Self::list_passkeys(&mailing.relying, record);
        let record = Self::list_client(mailing.forwarded, record);
        let admin = match &mailing.admin {
            Some(link) => format!(
                "{link} (shown once and kept nowhere; valid {}; opening it signs in as the owner)",
                settings.admin
            ),
            None => "not made (one was made before, or an account can sign in)".to_owned(),
        };
        Ok(record.field("admin link", Line::plain(admin)))
    }

    /// Serves until `stopped`, with the API's half, the mail sender, the edge's lookups and the
    /// loops' events beside the server.
    async fn serving(
        server: Server<Sites, Files, Log>,
        listener: tokio::net::TcpListener,
        stopped: impl std::future::Future<Output = ()> + Send,
        (api, alongside): (Option<Beside>, Alongside),
    ) -> Result<(), edge_axum::Error> {
        let Alongside { back, mut told } = alongside;
        let log = Log(Mutex::new(std::io::stderr()));
        // ends when every loop has stopped and dropped its sender
        let telling = async {
            while let Some(event) = told.recv().await {
                log.tasked(&event);
            }
        };
        let looking = async {
            if let Some(back) = back {
                back.run().await;
            }
        };
        match api {
            Some((local, courier, mail)) => {
                let answering = async { tokio::join!(server.run(listener, stopped), local.run(), looking, telling).0 };
                // the sender rests on the system clock and never ends by itself
                tokio::select! {
                    done = answering => done,
                    () = courier.run(&mail) => Ok(()),
                }
            }
            None => tokio::join!(server.run(listener, stopped), looking, telling).0,
        }
    }

    /// Binds, prints the address, and serves until Ctrl-C.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let folder = self.folder.as_deref().unwrap_or_else(|| std::path::Path::new(DEFAULT_OUT));
        let deploys = Deploys::new(folder, self.host_mode.into()).map_err(|source| ServeError::Deploys { source })?;
        let mode = Self::mode(&deploys);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| ServeError::Run { source })?;
        let (listener, address) = self.bind(&runtime)?;
        let site = self.origin.clone().unwrap_or_else(|| format!("http://localhost:{}", address.port()));
        let mut product = self
            .data
            .as_deref()
            .map(|data| Product::open(&self, data, &site))
            .transpose()
            .map_err(|source| ServeError::Product { source })?;
        let fronts = product.as_ref().map(|product| product.fronts.clone());
        let back = product.as_mut().and_then(|product| product.back.take());
        let server = Self::server(fronts, deploys)?;
        io.json(&Started { schema: "uses.serve/v1", address: format!("http://{address}"), mode })?;
        io.result(&Answer::of(format_args!("http://{address}")))?;
        let (events, told) = Events::channel();
        let tasks = product.as_ref().map(|product| self.start_tasks(product, &events));
        drop(events);
        let (server, local) = self.attach(server, product, io)?;
        let signal = Mutex::new(None);
        let stopped = async {
            if let Err(source) = tokio::signal::ctrl_c().await {
                *signal.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(source);
            }
            if let Some(tasks) = &tasks {
                tasks.halt();
            }
        };
        let serving = Self::serving(server, listener, stopped, (local, Alongside { back, told }));
        runtime.block_on(serving).map_err(|source| ServeError::Server { source })?;
        if let Some(tasks) = tasks {
            tasks.join();
        }
        Self::ended(signal.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    /// How the folders are served, as the start line names it.
    fn mode(deploys: &Deploys) -> &'static str {
        match deploys.mode() {
            Mode::Single => "single",
            _ => "folders",
        }
    }

    /// The server over the control plane's sites, when there is one, and the folders.
    fn server(fronts: Option<Fronts>, deploys: Deploys) -> Result<Server<Sites, Files, Log>, Error> {
        let edge = Edge::new(Fallback::new(fronts, deploys), Files, Limits::default())
            .map_err(|source| ServeError::Edge { source })?;
        Server::new(edge, Log(Mutex::new(std::io::stderr())), Scheme::Plain)
            .map_err(|source| Error::from(ServeError::Server { source }))
    }

    /// Binds the listener and says where it is.
    fn bind(
        &self,
        runtime: &tokio::runtime::Runtime,
    ) -> Result<(tokio::net::TcpListener, std::net::SocketAddr), Error> {
        let listen = |source| Error::from(ServeError::Listen { address: self.address, source });
        let listener = runtime.block_on(tokio::net::TcpListener::bind(self.address)).map_err(listen)?;
        let address = listener.local_addr().map_err(listen)?;
        Ok((listener, address))
    }

    /// How the run ended: done, or by the signal handler's failure.
    fn ended(signal: Option<std::io::Error>) -> Result<Exit, Error> {
        match signal {
            Some(source) => Err(ServeError::Run { source }.into()),
            None => Ok(Exit::Done),
        }
    }
}
