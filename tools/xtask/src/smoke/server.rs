//! The `ink serve` of a stage: a child process whose output goes to a file, and which is killed
//! when it is dropped.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::Fault;

/// A running `ink serve`.
#[derive(Debug)]
pub(super) struct Server {
    child: Child,
    log: PathBuf,
}

impl Server {
    /// How many one-second tries it gets to say it is listening.
    const TRIES: u64 = 30;

    /// Starts `ink` with `args` in `dir`, its output into `log`, and waits until it listens.
    pub(super) fn start(ink: &Path, (dir, log): (&Path, &Path), args: &[&str]) -> Result<Self, Fault> {
        let disk = |source| Fault::Disk { path: log.to_path_buf(), source };
        let out = File::create(log).map_err(disk)?;
        let err = out.try_clone().map_err(disk)?;
        let line = format!("ink {}", args.join(" "));
        let child = Command::new(ink)
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .spawn()
            .map_err(|source| Fault::Start { line, source })?;
        let mut server = Self { child, log: log.to_path_buf() };
        server.listening()?;
        Ok(server)
    }

    /// What it has said so far.
    pub(super) fn said(&self) -> Result<String, Fault> {
        std::fs::read_to_string(&self.log).map_err(|source| Fault::Disk { path: self.log.clone(), source })
    }

    /// Ends it and waits for it.
    pub(super) fn stop(mut self) -> Result<(), Fault> {
        self.end().map_err(|source| Fault::Disk { path: self.log.clone(), source })
    }

    fn end(&mut self) -> std::io::Result<()> {
        self.child.kill()?;
        self.child.wait().map(drop)
    }

    /// Waits for the line that says the listener is up, or for the child to end.
    fn listening(&mut self) -> Result<(), Fault> {
        for _ in 0..Self::TRIES {
            let log = self.said()?;
            if log.contains("listening on") {
                return Ok(());
            }
            let ended = self.child.try_wait().map_err(|source| Fault::Disk { path: self.log.clone(), source })?;
            if ended.is_some() {
                return Err(Fault::Exited { log });
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        Err(Fault::Unready { seconds: Self::TRIES, log: self.said()? })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // a child that already ended is no failure of the run that is being left
        drop(self.end());
    }
}
