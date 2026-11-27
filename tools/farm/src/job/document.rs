//! The machine report, schema `uses.farm/v1`.

use serde::Serialize;

use crate::job::summary::Summary;

/// `uses.farm/v1`: what `--json` prints for a job.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Wire {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) command: String,
    pub(crate) host: String,
    pub(crate) slot: Option<u32>,
    pub(crate) queued_secs: u64,
    /// `passed`, `failed`, `timed out`, `running`, `aborted` or `lost`.
    pub(crate) outcome: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) ran_secs: Option<u64>,
    pub(crate) log: String,
    pub(crate) summary: Summary,
    /// `ran`, `cached` or `attached`.
    pub(crate) served: &'static str,
}

/// The whole report of one command: `uses.farm/v1`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Document {
    pub(crate) schema: &'static str,
    /// The exit code the command ends with.
    pub(crate) exit: u8,
    pub(crate) jobs: Vec<Wire>,
    /// Files a bless changed, relative to the workspace.
    pub(crate) blessed: Vec<String>,
    /// Files a fmt changed, relative to the workspace.
    pub(crate) formatted: Vec<String>,
}

impl Document {
    pub(crate) const SCHEMA: &'static str = "uses.farm/v1";
}
