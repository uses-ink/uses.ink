//! `ink serve [folder]`: the deploy `ink build` wrote, served by the edge crate.
//!
//! `ink preview` builds the site in memory and serves it for writing. This serves what was
//! already built, through the same `edge` code the hosted product runs: its refusals, budgets,
//! conditional and range handling and the manifest's headers, unchanged.

use std::sync::{Arc, Mutex};

use edge::{Edge, Limits, Scheme};
use edge_axum::{Bridge, Local, Mount, Server};
use edge_control::{Fallback, Fronts};
use edge_folder::{Deploys, Files, Mode};

use crate::command::remote::{Fetching, Settings};
use crate::command::source::DEFAULT_OUT;
use crate::grammar::args::{self, HostMode};
use crate::output::error::{Error, ServeError};
use crate::output::exit::Exit;
use crate::output::term::Answer;
use crate::output::{Io, Say};
use crate::selfhost::product::{Mailing, Product};
use crate::selfhost::report::log::Logger;
use crate::selfhost::report::startup::{Admin, Frame, Hosted, Plan, Span, Stage, Startup};
use crate::selfhost::signin::Sender;
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

impl HostMode {
    /// The mode as `--host-mode` writes it.
    const fn word(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Single => "single",
            Self::Folders => "folders",
        }
    }
}

/// What runs beside the server when there is an API: the half that answers it, and the mail
/// sender with where it reports.
type Beside = (Local<crate::selfhost::product::Wired>, edge_api::Courier<Sender, clock::System>, Logger);

/// The sites a request's host picks from: the control plane's, then the folder's.
type Sites = Fallback<Fronts, Deploys>;

/// The server, and what runs beside it when `--data` turned the API on.
type Attached = (Server<Sites, Files, Logger>, Option<Beside>);

/// What runs beside the server and the API: the edge's lookups and the loops' events.
struct Alongside {
    back: Option<crate::selfhost::product::Lookup>,
    told: tokio::sync::mpsc::UnboundedReceiver<Tasked>,
    log: Logger,
}

impl args::Serve {
    /// The server with the API mounted, when `--data` turned it on, and the half that answers it.
    fn attach(&self, server: Server<Sites, Files, Logger>, product: Option<Product>) -> Attached {
        let (Some(_), Some(product)) = (&self.data, product) else { return (server, None) };
        let (mount, local) = self.mount(product);
        (server.with_api(mount), Some(local))
    }

    /// The API's way into the server, and the half that answers it.
    fn mount(&self, product: Product) -> (Mount, Beside) {
        let (bridge, local) = Bridge::new(product.api, product.limits);
        let Mailing { courier, told, forwarded, .. } = product.mailing;
        let bridge = bridge.forwarded(forwarded);
        (Mount { host: self.api_host.to_ascii_lowercase().into(), bridge }, (local, courier, told))
    }

    /// The report of this start: what it runs with, and the first-run link when this start made
    /// it. Built from the product that was set up, so it says what runs.
    fn startup(&self, frame: Frame<'_>, product: Option<&Product>) -> Result<Startup, Error> {
        let (Some(data), Some(product)) = (&self.data, product) else { return Ok(Startup::new(frame, None)) };
        let admin = match &product.mailing.admin {
            Some(link) => {
                Admin::Issued { link: link.to_string(), valid: Span(product.mailing.settings.admin.duration()) }
            }
            None => Admin::Kept,
        };
        let plan = Plan::running(self, product).map_err(|source| ServeError::Product { source })?;
        let hosted = Hosted {
            plan,
            site: product.site.clone(),
            data: data.clone(),
            operators: product.operators.clone(),
            admin,
            retired: product.retired.is_some(),
        };
        Ok(Startup::new(frame, Some(&hosted)))
    }

    /// What a start would run with, worked out from the flags alone: nothing is bound, made or
    /// written, so the data folder and the first-run link are left as they are.
    fn planned(&self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let frame = Frame { serve: self, address: self.address, mode: self.host_mode.word(), stage: Stage::Planned };
        let site = self.origin.clone().unwrap_or_else(|| format!("http://localhost:{}", self.address.port()));
        let hosted = match &self.data {
            Some(data) => Some(Hosted {
                plan: Plan::planned(self, &site).map_err(|source| ServeError::Product { source })?,
                site,
                data: data.clone(),
                operators: data.join("operator-keys"),
                admin: Admin::Unminted,
                retired: data.join("owner-token").exists(),
            }),
            None => None,
        };
        let startup = Startup::new(frame, hosted.as_ref());
        io.json(&startup)?;
        io.result(&startup)?;
        Ok(Exit::Done)
    }

    /// Starts the watch and the worker over `product`'s control plane.
    fn start_tasks(&self, product: &Product, events: &Events) -> Tasks {
        let mut tasks = Tasks::new();
        self.start_watch(&mut tasks, product, events);
        self.start_worker(&mut tasks, product, events);
        self.start_domains(&mut tasks, product, events);
        tasks
    }

    /// Reads the proof record of every custom domain that is due, once an hour (decision 0151).
    fn start_domains(&self, tasks: &mut Tasks, product: &Product, events: &Events) {
        let (ports, origin, policy) = (Arc::clone(&product.ports), product.origin.clone(), self.hosting.policy());
        let resolver = match self.hosting.resolving() {
            Ok(resolver) => resolver,
            Err(error) => {
                events.tell(Tasked::Failed { cause: edge_control::Chain::of(&error) });
                return;
            }
        };
        tasks.start(Task::Domains, (self.hosting.restart(), events), move |clock: Stoppable, events: Events| {
            let (ports, origin, resolver) = (Arc::clone(&ports), origin.clone(), resolver.clone());
            Box::pin(async move {
                let control = control::Control::new(ports, origin).with_policy(policy);
                let dns = authority_dns::Dns::new(resolver);
                loop {
                    match dns.recheck(&control, clock::Clock::now(&clock), 100).await {
                        Ok(found) if found.quiet() => {}
                        Ok(found) => events.tell(Tasked::Rechecked(found)),
                        Err(error) => events.tell(Tasked::Failed { cause: edge_control::Chain::of(&error) }),
                    }
                    if clock::Clock::sleep(&clock, std::time::Duration::from_hours(1)).await == clock::Slept::Stopped {
                        return;
                    }
                }
            })
        });
    }

    fn start_watch(&self, tasks: &mut Tasks, product: &Product, events: &Events) {
        let (ports, origin, interval) = (Arc::clone(&product.ports), product.origin.clone(), product.interval);
        let (policy, previews) = (self.hosting.policy(), self.hosting.previews());
        let git = self.hosting.git_cache.clone().unwrap_or_else(|| product.data.join("git"));
        let (heads, network, https) = (git.join("heads"), self.hosting.network(), product.https.clone());
        tasks.start(Task::Watch, (self.hosting.restart(), events), move |clock: Stoppable, events: Events| {
            let (ports, origin) = (Arc::clone(&ports), origin.clone());
            let heads = watch_git::Remotes::new(heads.clone(), network).with_https(https.clone());
            Box::pin(async move {
                let control = control::Control::new(ports, origin).with_policy(policy);
                let mut watch =
                    watch::Watch::new(&control, heads, (clock, clock::System), interval).with_previews(previews);
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

    /// Serves until `stopped`, with the API's half, the mail sender, the edge's lookups and the
    /// loops' events beside the server.
    async fn serving(
        server: Server<Sites, Files, Logger>,
        listener: tokio::net::TcpListener,
        stopped: impl std::future::Future<Output = ()> + Send,
        (api, alongside): (Option<Beside>, Alongside),
    ) -> Result<(), edge_axum::Error> {
        let Alongside { back, mut told, log } = alongside;
        // ends when every loop has stopped and dropped its sender
        let telling = async {
            while let Some(event) = told.recv().await {
                log.tell(&event);
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

    /// Binds, prints the address, and serves until Ctrl-C. With `--print-config` it only says
    /// what it would run with.
    #[expect(clippy::too_many_lines, reason = "one sequence of steps, each a line")]
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        if self.print_config {
            return self.planned(io);
        }
        let folder = self.folder.as_deref().unwrap_or_else(|| std::path::Path::new(DEFAULT_OUT));
        let deploys = Deploys::new(folder, self.host_mode.into()).map_err(|source| ServeError::Deploys { source })?;
        let mode = Self::mode(&deploys);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| ServeError::Run { source })?;
        let (listener, address) = self.bind(&runtime)?;
        let site = self.origin.clone().unwrap_or_else(|| format!("http://localhost:{}", address.port()));
        let log = Logger::new(io, &self);
        let mut product = self
            .data
            .as_deref()
            .map(|data| Product::open(&self, data, (&site, log.clone())))
            .transpose()
            .map_err(|source| ServeError::Product { source })?;
        let fronts = product.as_ref().map(|product| product.fronts.clone());
        let back = product.as_mut().and_then(|product| product.back.take());
        let server = Self::server(fronts, deploys, log.clone())?;
        let startup = self.startup(Frame { serve: &self, address, mode, stage: Stage::Serving }, product.as_ref())?;
        io.json(&startup)?;
        io.result(&Answer::of(format_args!("http://{address}")))?;
        io.aside(Say::Warnings, &startup)?;
        let (events, told) = Events::channel();
        let tasks = product.as_ref().map(|product| self.start_tasks(product, &events));
        drop(events);
        let (server, local) = self.attach(server, product);
        let signal = Mutex::new(None);
        let stopped = async {
            if let Err(source) = tokio::signal::ctrl_c().await {
                *signal.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(source);
            }
            if let Some(tasks) = &tasks {
                tasks.halt();
            }
        };
        let serving = Self::serving(server, listener, stopped, (local, Alongside { back, told, log }));
        runtime.block_on(serving).map_err(|source| ServeError::Server { source })?;
        if let Some(tasks) = tasks {
            tasks.join();
        }
        Self::ended(signal.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    /// How the folders are served, as the report names it.
    fn mode(deploys: &Deploys) -> &'static str {
        match deploys.mode() {
            Mode::Single => "single",
            _ => "folders",
        }
    }

    /// The server over the control plane's sites, when there is one, and the folders.
    fn server(fronts: Option<Fronts>, deploys: Deploys, log: Logger) -> Result<Server<Sites, Files, Logger>, Error> {
        let edge = Edge::new(Fallback::new(fronts, deploys), Files, Limits::default())
            .map_err(|source| ServeError::Edge { source })?;
        Server::new(edge, log, Scheme::Plain).map_err(|source| Error::from(ServeError::Server { source }))
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
