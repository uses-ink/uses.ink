//! A transport that answers from a script and remembers what it was asked.

use std::collections::VecDeque;
use std::os::unix::process::ExitStatusExt as _;
use std::process::ExitStatus;
use std::sync::Mutex;

use crate::host::transport::process::Failure;
use crate::host::transport::program::Call;
use crate::host::transport::{Done, Pull, Push, Transport};
use crate::host::{Host, Via};

/// One scripted answer to a call.
#[derive(Debug, Clone)]
pub(crate) struct Reply {
    pub(crate) lines: Vec<String>,
    pub(crate) code: i32,
}

impl Reply {
    /// What a probe of a two-slot host answers.
    pub(crate) const PROBE: &[&str] =
        &["now 1", "arch x86_64", "cores 12", "load 0.5", "slots 2", "slot 0 free", "slot 1 busy", "queued 0"];

    pub(crate) fn ok(lines: &[&str]) -> Self {
        Self { lines: lines.iter().map(|line| (*line).to_owned()).collect(), code: 0 }
    }
}

/// Answers calls from a queue, in order, and records them.
#[derive(Debug, Default)]
pub(crate) struct Fake {
    pub(crate) replies: Mutex<VecDeque<Reply>>,
    pub(crate) calls: Mutex<Vec<Call>>,
    pub(crate) pushed: Mutex<Vec<String>>,
    pub(crate) pulled: Mutex<Vec<String>>,
    pub(crate) local: bool,
}

impl Fake {
    /// A host reached over ssh, named `name`.
    pub(crate) fn remote(name: &str) -> Host {
        Host { name: name.parse().unwrap(), via: Via::Ssh("x".parse().unwrap()), ..Host::local() }
    }

    pub(crate) fn saying(replies: Vec<Reply>) -> Self {
        Self { replies: Mutex::new(replies.into()), ..Self::default() }
    }
}

impl Transport for Fake {
    fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        self.calls.lock().unwrap().push(call.clone());
        let reply = self.replies.lock().unwrap().pop_front().unwrap_or(Reply { lines: vec![], code: 0 });
        reply.lines.iter().for_each(|line| lines(line));
        Ok(Done { status: ExitStatus::from_raw(reply.code << 8), stderr: String::new() })
    }

    fn push(&self, push: &Push<'_>) -> Result<(), Failure> {
        self.pushed.lock().unwrap().push(push.tree.to_owned());
        Ok(())
    }

    fn pull(&self, pull: &Pull<'_>) -> Result<(), Failure> {
        self.pulled.lock().unwrap().push(pull.files.to_owned());
        Ok(())
    }

    fn is_local(&self) -> bool {
        self.local
    }
}
