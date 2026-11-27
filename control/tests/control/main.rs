//! The control plane's use cases on real adapters: SQLite in memory, a directory in a throwaway
//! folder, the queue in SQLite rows and a freshly keyed sealer. Nothing sleeps: every call is
//! told what time it is.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_control as control;

mod credential;
mod deploy;
mod export;
mod job;
mod remove;
mod site;
mod world;
