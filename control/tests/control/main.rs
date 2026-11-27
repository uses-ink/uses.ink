//! The control plane's use cases on real adapters: SQLite in memory, a directory in a throwaway
//! folder, the queue in SQLite rows and a freshly keyed sealer. Nothing sleeps: every call is
//! told what time it is.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise and read top to bottom"
)]

extern crate uses_ink_control as control;

mod credential;
mod deploy;
mod export;
mod job;
mod read;
mod remove;
mod signin;
mod site;
mod world;
