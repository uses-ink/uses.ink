//! What a release step can fail with.

use std::fmt;

use crate::process::Failure;

/// Why a release step failed.
#[derive(Debug)]
pub(crate) enum Fault {
    /// A command that did not end with success.
    Command { line: String, source: Failure },
    /// `dist` is not on `PATH`.
    NoDist,
    /// Neither `docker` nor `podman` is on `PATH`.
    NoRuntime,
    /// The image's configuration is not what the guide promises.
    Config { field: &'static str, found: String, wanted: &'static str },
    /// The container ended before it answered.
    Exited { logs: String },
    /// The container was running and never answered.
    Unready { seconds: u64, logs: String },
    /// A folder `dist` should have written cannot be read.
    Folder { path: String, source: std::io::Error },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { line, .. } => write!(out, "`{line}` failed"),
            Self::NoDist => {
                write!(
                    out,
                    "`dist` is not on PATH; install it with `cargo install cargo-dist --version 0.34.0 --locked`"
                )
            }
            Self::NoRuntime => write!(out, "neither `docker` nor `podman` is on PATH"),
            Self::Config { field, found, wanted } => write!(out, "the image's {field} is `{found}`, not {wanted}"),
            Self::Exited { logs } => write!(out, "the container ended before it answered, saying:\n{logs}"),
            Self::Unready { seconds, logs } => {
                write!(out, "the container did not answer `/healthz` in {seconds} s, saying:\n{logs}")
            }
            Self::Folder { path, .. } => write!(out, "cannot read {path}"),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Command { source, .. } => Some(source),
            Self::Folder { source, .. } => Some(source),
            Self::NoDist | Self::NoRuntime | Self::Config { .. } | Self::Exited { .. } | Self::Unready { .. } => None,
        }
    }
}
