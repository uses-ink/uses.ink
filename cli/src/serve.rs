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

use crate::args::{self, HostMode};
use crate::error::Error;
use crate::exit::Exit;
use crate::output::Io;
use crate::product::{Mailing, Product};
use crate::remote::{Fetching, Settings};
use crate::signin::{Sender, Told, Transport};
use crate::tasks::{Event as Tasked, Events, Stoppable, Task, Tasks};

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
type Beside = (Local<crate::product::Wired>, edge_api::Courier<Sender, clock::System>, Told);

/// The sites a request's host picks from: the control plane's, then the folder's.
type Sites = Fallback<Fronts, Deploys>;

/// The server, and what runs beside it when `--data` turned the API on.
type Attached = (Server<Sites, Files, Log>, Option<Beside>);

/// What runs beside the server and the API: the edge's lookups and the loops' events.
struct Alongside {
    back: Option<crate::product::Lookup>,
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
            io.line(format_args!("api: off; give --data <folder> to turn on the control plane and its API"))?;
            return Ok((server, None));
        };
        let (mount, local) = self.start_api(product, data, io)?;
        Ok((server.with_api(mount), Some(local)))
    }

    /// Sets the control plane up, prints every setting it runs with, and returns the API's way
    /// into the server and the half that answers.
    fn start_api(&self, product: Product, data: &std::path::Path, io: &mut Io<'_>) -> Result<(Mount, Beside), Error> {
        let (limits, interval) = (product.limits, product.interval);
        io.line(format_args!("api: http://{}/v1/sites (host {})", self.api_host, self.api_host))?;
        io.line(format_args!("data: {}", data.display()))?;
        match &product.fresh {
            Some(token) => io.line(format_args!(
                "owner token: {} (shown once; kept in {})",
                token.reveal(),
                product.token.display()
            ))?,
            None => io.line(format_args!("owner token: in {}", product.token.display()))?,
        }
        io.line(format_args!(
            "sites: <handle>.{}",
            self.origin.trim_start_matches("http://").trim_start_matches("https://")
        ))?;
        io.line(format_args!(
            "limits: json body {} B, upload {} B, {} files, {} B a file, export {} B, lists {} rows",
            limits.body, limits.archive, limits.files, limits.file, limits.export, limits.rows
        ))?;
        self.describe_loops(interval, io)?;
        self.list_signin(&product.mailing, io)?;
        let (bridge, local) = Bridge::new(product.api, limits);
        let Mailing { courier, told, .. } = product.mailing;
        Ok((Mount { host: self.api_host.to_ascii_lowercase().into(), bridge }, (local, courier, told)))
    }

    /// Prints what the watch and the worker run with.
    fn describe_loops(&self, interval: watch::Interval, io: &mut Io<'_>) -> Result<(), Error> {
        let schemes: Vec<String> = self.hosting.schemes().iter().map(ToString::to_string).collect();
        io.line(format_args!(
            "poll: every {}s, backing off to {}s, a new site within {}s; git over {}",
            interval.base().as_secs(),
            interval.max().as_secs(),
            interval.lookout().as_secs(),
            schemes.join(" and ")
        ))?;
        let (restart, (lease, idle)) = (self.hosting.restart(), self.hosting.worker());
        io.line(format_args!(
            "worker: lease {}s, rests {}s; a loop that crashes restarts after {}s, {} times in a row",
            lease.as_secs(),
            idle.as_secs(),
            restart.delay.as_secs(),
            restart.limit
        ))?;
        Ok(())
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
        let git = self.hosting.git_cache.clone().unwrap_or_else(|| product.data.join("git"));
        let (heads, network, https) = (git.join("heads"), self.hosting.network(), product.https.clone());
        tasks.start(Task::Watch, (self.hosting.restart(), events), move |clock: Stoppable, events: Events| {
            let (ports, origin) = (Arc::clone(&ports), origin.clone());
            let heads = watch_git::Remotes::new(heads.clone(), network).with_https(https.clone());
            Box::pin(async move {
                let control = control::Control::new(ports, origin);
                let mut watch = watch::Watch::new(&control, heads, (clock, clock::System), interval);
                watch.run(&events).await;
            })
        });
    }

    fn start_worker(&self, tasks: &mut Tasks, product: &Product, events: &Events) {
        let (ports, origin) = (Arc::clone(&product.ports), product.origin.clone());
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
            let remote = Fetching::new(control::Control::new(Arc::clone(&ports), origin.clone()), settings);
            Box::pin(async move {
                let stored = match blob_dir::Directory::open(&blobs) {
                    Ok(stored) => stored,
                    Err(error) => {
                        events.tell(Tasked::Failed { cause: edge_control::Chain::of(&error) });
                        return;
                    }
                };
                let control = control::Control::new(ports, origin);
                let builder = build_local::Local::new(stored).with_remote(remote);
                watch::Worker::new(&control, builder, clock, times).run(&events).await;
            })
        });
    }

    /// Prints where mail goes.
    fn list_mail(&self, io: &mut Io<'_>) -> Result<(), Error> {
        let flags = &self.signin;
        match flags.mail {
            Transport::Stdout => io.line(format_args!("mail: stdout (messages and their links are printed here)"))?,
            Transport::Smtp => io.line(format_args!(
                "mail: smtp {}:{} {:?} from {} (password {})",
                flags.smtp_host.as_deref().unwrap_or_default(),
                flags.smtp_port.unwrap_or(0),
                flags.smtp_security,
                flags.mail_from.as_deref().unwrap_or_default(),
                if flags.smtp_user.is_some() { "set, never printed" } else { "none" },
            ))?,
        }
        Ok(())
    }

    /// Prints every sign-in setting it runs with, and the first-run link when this start made it.
    fn list_signin(&self, mailing: &Mailing, io: &mut Io<'_>) -> Result<(), Error> {
        let flags = &self.signin;
        let settings = &mailing.settings;
        io.line(format_args!(
            "sign-in: link {}, session {}, sign-up {}, link base {}",
            settings.login, settings.session, settings.signup, mailing.base
        ))?;
        self.list_mail(io)?;
        io.line(format_args!(
            "ask limits: {} per address, {} per source, {} tracked, outbox {}",
            mailing.rates.address, mailing.rates.source, mailing.rates.tracked, mailing.outbox
        ))?;
        io.line(format_args!(
            "cookie: {} path {} ({})",
            flags.jar(mailing.secure).map_err(|source| Error::Product { source })?.name(),
            flags.cookie_path.as_deref().unwrap_or("/"),
            if mailing.secure { "Secure" } else { "plain http, not Secure" }
        ))?;
        match &mailing.admin {
            Some(link) => io.line(format_args!(
                "admin link: {link} (shown once and kept nowhere; valid {}; opening it signs in as the owner)",
                settings.admin
            ))?,
            None => io.line(format_args!("admin link: not made (one was made before, or an account can sign in)"))?,
        }
        Ok(())
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
        let deploys = Deploys::new(&self.folder, self.host_mode.into()).map_err(|source| Error::Deploys { source })?;
        let mode = Self::mode(&deploys);
        let mut product = self
            .data
            .as_deref()
            .map(|data| Product::open(&self, data))
            .transpose()
            .map_err(|source| Error::Product { source })?;
        let fronts = product.as_ref().map(|product| product.fronts.clone());
        let back = product.as_mut().and_then(|product| product.back.take());
        let server = Self::server(fronts, deploys)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| Error::Run { source })?;
        let (listener, address) = self.bind(&runtime)?;
        io.json(&Started { schema: "uses.serve/v1", address: format!("http://{address}"), mode })?;
        io.line(format_args!("http://{address}"))?;
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
        runtime.block_on(serving).map_err(|source| Error::Server { source })?;
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
            .map_err(|source| Error::Edge { source })?;
        Server::new(edge, Log(Mutex::new(std::io::stderr())), Scheme::Plain).map_err(|source| Error::Server { source })
    }

    /// Binds the listener and says where it is.
    fn bind(
        &self,
        runtime: &tokio::runtime::Runtime,
    ) -> Result<(tokio::net::TcpListener, std::net::SocketAddr), Error> {
        let listen = |source| Error::Listen { address: self.address, source };
        let listener = runtime.block_on(tokio::net::TcpListener::bind(self.address)).map_err(listen)?;
        let address = listener.local_addr().map_err(listen)?;
        Ok((listener, address))
    }

    /// How the run ended: done, or by the signal handler's failure.
    fn ended(signal: Option<std::io::Error>) -> Result<Exit, Error> {
        match signal {
            Some(source) => Err(Error::Run { source }),
            None => Ok(Exit::Done),
        }
    }
}
