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

mod abuse;
mod budget;
mod codes;
mod credential;
mod deploy;
mod domain;
mod export;
mod grace;
mod handle;
mod job;
mod keys;
mod org;
mod preview;
mod read;
mod remove;
mod signin;
mod site;
mod world;
