//! `ink serve [folder]`: the deploy `ink build` wrote, served by the edge crate.
//!
//! `ink preview` builds the site in memory and serves it for writing. This serves what was
//! already built, through the same `edge` code the hosted product runs: its refusals, budgets,
//! conditional and range handling and the manifest's headers, unchanged.

use std::io::Write;
use std::sync::Mutex;

use edge::{Edge, Limits, Scheme};
use edge_axum::{Event, Observe, Server};
use edge_folder::{Deploys, Files, Mode};
use serde::Serialize;

use crate::args::{self, HostMode};
use crate::error::Error;
use crate::exit::Exit;
use crate::output::Io;

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

impl args::Serve {
    /// Binds, prints the address, and serves until Ctrl-C.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let deploys = Deploys::new(&self.folder, self.host_mode.into()).map_err(|source| Error::Folder { source })?;
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
            .map_err(|source| Error::Serve { source })?;
        let listen = |source| Error::Listen { address: self.address, source };
        let listener = runtime.block_on(tokio::net::TcpListener::bind(self.address)).map_err(listen)?;
        let address = listener.local_addr().map_err(listen)?;
        io.json(&Started { schema: "uses.serve/v1", address: format!("http://{address}"), mode })?;
        io.line(format_args!("http://{address}"))?;
        let signal = Mutex::new(None);
        let stopped = async {
            if let Err(source) = tokio::signal::ctrl_c().await {
                *signal.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(source);
            }
        };
        runtime.block_on(server.run(listener, stopped)).map_err(|source| Error::Server { source })?;
        match signal.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner) {
            Some(source) => Err(Error::Serve { source }),
            None => Ok(Exit::Done),
        }
    }
}
