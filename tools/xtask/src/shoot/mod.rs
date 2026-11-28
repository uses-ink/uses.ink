//! `cargo xtask shoot <target> --width 1440,390`: the built pages drawn by a real headless browser,
//! so what ships can be looked at (decision 0240). It builds the sites with `ink build`, serves them
//! with `ink serve` on loopback, drives Chromium over them with the pinned packages of `tools/shoot/`
//! and writes a full-page PNG per page and width under `target/shots/`. Each picture is also printed
//! as base64 between `=====SHOT` and `=====END` lines, because the farm's log is the only way back
//! from a build host. Not a step of the gate: it is run by name, through `farm cargo xtask shoot`.

mod fault;
mod flags;
mod host;
mod plan;
mod rig;
mod serve;
mod session;
mod target;

use std::fmt;
use std::path::Path;

use crate::check::Step;
use crate::error::Error;
use crate::report::Report;

pub(crate) use fault::Fault;
pub(crate) use flags::Flags;
pub(crate) use target::Target;

/// One run: a target at some widths.
#[derive(Debug, Clone)]
pub(crate) struct Shoot {
    pub(crate) target: Target,
    pub(crate) flags: Flags,
}

impl Step for Shoot {
    fn run(&self, root: &Path, report: &mut Report) {
        if let Err(fault) = self.target.run(root, &self.flags, report) {
            report.fail(Error::Shot { target: self.target, fault });
        }
    }
}

impl fmt::Display for Shoot {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "cargo xtask shoot {}", self.target)
    }
}
