//! A server of the run: `ink serve`, started on loopback and stopped when the pictures are taken.

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::fault::Fault;

/// A running server, and the file its output goes to.
#[derive(Debug)]
pub(crate) struct Served {
    child: Child,
    log: PathBuf,
}

impl Served {
    /// How long a server gets to listen.
    const PATIENCE: Duration = Duration::from_secs(30);

    /// Starts `command` with its output in `log`, and returns when it listens on `address`.
    pub(crate) fn start(
        mut command: Command,
        name: &'static str,
        address: SocketAddr,
        log: &Path,
    ) -> Result<Self, Fault> {
        let file = |source| Fault::File { path: log.to_path_buf(), source };
        if let Some(dir) = log.parent() {
            std::fs::create_dir_all(dir).map_err(file)?;
        }
        let output = std::fs::File::create(log).map_err(file)?;
        command.stdin(Stdio::null()).stdout(output.try_clone().map_err(file)?).stderr(output);
        let program = command.get_program().to_string_lossy().into_owned();
        let child = command.spawn().map_err(|source| Fault::Spawn { program, source })?;
        let mut served = Self { child, log: log.to_path_buf() };
        let start = Instant::now();
        while start.elapsed() < Self::PATIENCE {
            if TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok() {
                return Ok(served);
            }
            if served.child.try_wait().map_err(file)?.is_some() {
                return Err(Fault::Exited { name, log: served.said() });
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(Fault::Unready { name, seconds: Self::PATIENCE.as_secs(), log: served.said() })
    }

    /// What the server printed, as far as it can be read.
    fn said(&self) -> String {
        match std::fs::read(&self.log) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => format!("(its log {} cannot be read: {error})", self.log.display()),
        }
    }

    /// Stops it, and says what stopping it did.
    pub(crate) fn stop(mut self) -> Result<(), Fault> {
        let file = |source| Fault::File { path: self.log.clone(), source };
        self.child.kill().map_err(file)?;
        self.child.wait().map_err(file)?;
        Ok(())
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        // a server already stopped by `stop` or ended by itself has nothing left to kill; this only
        // covers a run that failed before `stop`, and there is no one left to tell
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

/// Every server of a run.
#[derive(Debug, Default)]
pub(crate) struct Servers(Vec<Served>);

impl Servers {
    /// Adds a server.
    pub(crate) fn add(&mut self, served: Served) {
        self.0.push(served);
    }

    /// Stops all of them, and fails with the first that did not stop.
    pub(crate) fn stop(self) -> Result<(), Fault> {
        let mut first = Ok(());
        for served in self.0 {
            if let Err(fault) = served.stop() {
                first = first.and(Err(fault));
            }
        }
        first
    }
}
