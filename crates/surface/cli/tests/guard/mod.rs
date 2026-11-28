//! A spawned `ink serve` that cannot outlive the test: the guard owns the child from the moment
//! it exists, so a panic anywhere after (a failed assertion, a line that never came) still kills
//! and reaps it.

#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    reason = "shared by several test crates; tests stop at the first surprise"
)]

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

/// A running child process, killed and waited for when dropped, also on unwind.
pub struct Guard {
    child: Child,
}

impl Guard {
    /// Spawns `command` with piped output and no input.
    pub fn spawn(command: &mut Command) -> Self {
        let child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        Self { child }
    }

    /// A child spawned by the caller, which wants its own output handling.
    pub fn adopt(child: Child) -> Self {
        Self { child }
    }

    /// Whether the child has already ended.
    pub fn ended(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    /// The child's stdout, once.
    pub fn stdout(&mut self) -> ChildStdout {
        self.child.stdout.take().unwrap()
    }

    /// The child's stderr, once.
    pub fn stderr(&mut self) -> ChildStderr {
        self.child.stderr.take().unwrap()
    }

    /// Both outputs, line by line, on one channel, read by threads that end with the child.
    pub fn lines(&mut self) -> Receiver<String> {
        let (into, received) = channel();
        Self::pump(self.stdout(), &into);
        Self::pump(self.stderr(), &into);
        received
    }

    fn pump(stream: impl Read + Send + 'static, into: &Sender<String>) {
        let into = into.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if into.send(line).is_err() {
                    return;
                }
            }
        });
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}
