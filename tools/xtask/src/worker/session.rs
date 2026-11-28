//! `wrangler dev --local` for one Worker crate, held while calls are made and stopped after.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use super::build::Crate;
use super::fault::Fault;
use super::loopback::Loopback;
use super::shell::Shell;

/// A running `wrangler dev`, its output kept as the last lines.
#[derive(Debug)]
pub(super) struct Session {
    child: Child,
    lines: Receiver<String>,
    tail: VecDeque<String>,
    pub(super) address: Loopback,
}

impl Session {
    /// How many lines of its output are kept.
    const KEPT: usize = 80;

    /// How long it may take to be ready: the Worker is already built, so this is workerd's start.
    const READY: Duration = Duration::from_secs(180);

    /// Starts `wrangler dev` for `krate` on `port`, with its state under `target/hosted/state`.
    pub(super) fn start(krate: &Crate<'_>, port: u16) -> Result<Self, Fault> {
        let shell = krate.shell;
        let state = shell.root().join(Shell::HOME).join("state").join(krate.dir.replace('/', "_"));
        // a run starts from nothing: the cases assume empty tables
        std::fs::remove_dir_all(&state).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }).map_err(|source| {
            Fault::File { path: state.clone(), source }
        })?;
        let mut command = shell.command("wrangler", &krate.path());
        command
            .args(["dev", "--local", "--ip", "127.0.0.1", "--port"])
            .arg(port.to_string())
            .arg("--inspector-port")
            .arg((port + 1000).to_string())
            .arg("--persist-to")
            .arg(&state)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let program = Shell::name(&command);
        let mut child = command.spawn().map_err(|source| Fault::Spawn { program, source })?;
        let (send, lines) = channel();
        if let Some(out) = child.stdout.take() {
            let send = send.clone();
            std::thread::spawn(move || BufReader::new(out).lines().map_while(Result::ok).for_each(|line| drop(send.send(line))));
        }
        if let Some(err) = child.stderr.take() {
            std::thread::spawn(move || BufReader::new(err).lines().map_while(Result::ok).for_each(|line| drop(send.send(line))));
        }
        let mut session = Self { child, lines, tail: VecDeque::new(), address: Loopback { port } };
        session.ready()?;
        Ok(session)
    }

    /// Waits for the line that says the server listens.
    fn ready(&mut self) -> Result<(), Fault> {
        let until = Instant::now() + Self::READY;
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    let ready = line.contains("Ready on");
                    self.keep(line);
                    if ready {
                        return Ok(());
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        Err(Fault::Ready { log: self.log() })
    }

    fn keep(&mut self, line: String) {
        if self.tail.len() == Self::KEPT {
            self.tail.pop_front();
        }
        self.tail.push_back(line);
    }

    /// The Worker's output so far, last lines.
    pub(super) fn log(&mut self) -> String {
        while let Ok(line) = self.lines.try_recv() {
            self.keep(line);
        }
        self.tail.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

impl Drop for Session {
    /// Stops wrangler and the workerd it started: the whole process group.
    fn drop(&mut self) {
        let group = format!("-{}", self.child.id());
        // a failure to signal leaves a process the farm kills with the job's group, so it is not an error here
        drop(std::process::Command::new("kill").args(["-TERM", "--", &group]).status());
        for _ in 0..50 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        drop(std::process::Command::new("kill").args(["-KILL", "--", &group]).status());
        drop(self.child.wait());
    }
}
