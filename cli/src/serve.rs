//! `ink serve [folder]`: the deploy `ink build` wrote, served by the edge crate.
//!
//! `ink preview` builds the site in memory and serves it for writing. This serves what was
//! already built, through the same `edge` code the hosted product runs: its refusals, budgets,
//! conditional and range handling and the manifest's headers, unchanged.

use std::io::Write;
use std::sync::Mutex;

use edge::{Edge, Limits, Scheme};
use edge_axum::{Bridge, Event, Local, Mount, Observe, Server};
use edge_folder::{Deploys, Files, Mode};
use serde::Serialize;

use crate::args::{self, HostMode};
use crate::error::Error;
use crate::exit::Exit;
use crate::output::Io;
use crate::product::{Mailing, Product};
use crate::signin::{Sender, Told, Transport};

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

/// The server, and what runs beside it when `--data` turned the API on.
type Attached = (Server<Deploys, Files, Log>, Option<Beside>);

impl args::Serve {
    /// The server with the API mounted, when `--data` turned it on, and the half that answers it.
    fn attach(&self, server: Server<Deploys, Files, Log>, io: &mut Io<'_>) -> Result<Attached, Error> {
        let Some(data) = &self.data else {
            io.line(format_args!("api: off; give --data <folder> to turn on the control plane and its API"))?;
            return Ok((server, None));
        };
        let (mount, local) = self.start_api(data, io)?;
        Ok((server.with_api(mount), Some(local)))
    }

    /// Sets the control plane up, prints every setting it runs with, and returns the API's way
    /// into the server and the half that answers.
    fn start_api(&self, data: &std::path::Path, io: &mut Io<'_>) -> Result<(Mount, Beside), Error> {
        let product = Product::open(self, data).map_err(|source| Error::Product { source })?;
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
        io.line(format_args!(
            "poll: every {}s, backing off to {}s (not running: no git remote adapter yet)",
            interval.base().as_secs(),
            interval.max().as_secs()
        ))?;
        self.list_signin(&product.mailing, io)?;
        let (bridge, local) = Bridge::new(product.api, limits);
        let Mailing { courier, told, .. } = product.mailing;
        Ok((Mount { host: self.api_host.to_ascii_lowercase().into(), bridge }, (local, courier, told)))
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

    /// Serves until `stopped`, with the API's half and the mail sender beside the server when
    /// there is an API.
    async fn serving(
        server: Server<Deploys, Files, Log>,
        listener: tokio::net::TcpListener,
        stopped: impl std::future::Future<Output = ()> + Send,
        local: Option<Beside>,
    ) -> Result<(), edge_axum::Error> {
        match local {
            Some((local, courier, told)) => {
                let answering = async { tokio::join!(server.run(listener, stopped), local.run()).0 };
                // the sender rests on the system clock and never ends by itself
                tokio::select! {
                    done = answering => done,
                    () = courier.run(&told) => Ok(()),
                }
            }
            None => server.run(listener, stopped).await,
        }
    }

    /// Binds, prints the address, and serves until Ctrl-C.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let deploys = Deploys::new(&self.folder, self.host_mode.into()).map_err(|source| Error::Deploys { source })?;
        let mode = match deploys.mode() {
            Mode::Single => "single",
            _ => "folders",
        };
        let edge = Edge::new(deploys, Files, Limits::default()).map_err(|source| Error::Edge { source })?;
        let server = Server::new(edge, Log(Mutex::new(std::io::stderr())), Scheme::Plain)
            .map_err(|source| Error::Server { source })?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| Error::Run { source })?;
        let listen = |source| Error::Listen { address: self.address, source };
        let listener = runtime.block_on(tokio::net::TcpListener::bind(self.address)).map_err(listen)?;
        let address = listener.local_addr().map_err(listen)?;
        io.json(&Started { schema: "uses.serve/v1", address: format!("http://{address}"), mode })?;
        io.line(format_args!("http://{address}"))?;
        let (server, local) = self.attach(server, io)?;
        let signal = Mutex::new(None);
        let stopped = async {
            if let Err(source) = tokio::signal::ctrl_c().await {
                *signal.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(source);
            }
        };
        let serving = Self::serving(server, listener, stopped, local);
        runtime.block_on(serving).map_err(|source| Error::Server { source })?;
        match signal.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner) {
            Some(source) => Err(Error::Run { source }),
            None => Ok(Exit::Done),
        }
    }
}
