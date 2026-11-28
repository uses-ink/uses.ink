//! `cargo xtask worker <case>`: Node, wrangler and workerd runs of the hosted spikes and tests,
//! with no Cloudflare account. A run installs the pinned npm packages under `target/hosted/`,
//! builds the case's Worker crate for `wasm32-unknown-unknown` with `worker-build`, measures it,
//! starts `wrangler dev --local`, makes its calls and reports what the Worker said. The gate's
//! stage `worker` runs the cases that are tests; the spikes are run by name.

mod app;
mod case;
mod fault;
mod migrate;
mod node;
mod probe;
mod serve;

use std::path::Path;

use crate::check::Step;
use crate::error::Error;
use crate::report::Report;

pub(crate) use case::Case;
pub(crate) use fault::Fault;

/// One case of the Worker runs, as a step of the gate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Worker(pub(crate) Case);

impl Step for Worker {
    fn run(&self, root: &Path, report: &mut Report) {
        if let Err(fault) = self.0.run(&node::Shell::new(root), report) {
            report.fail(Error::Worker { case: self.0, fault });
        }
    }
}

impl std::fmt::Display for Worker {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "cargo xtask worker {}", self.0)
    }
}
