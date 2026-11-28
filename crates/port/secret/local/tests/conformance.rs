//! The adapter against the port's conformance suite.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_secret_local as secret_local;

secret_conformance::suite!(|| (secret_local::Local::ephemeral().unwrap(), ()));
