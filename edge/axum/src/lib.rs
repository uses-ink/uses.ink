//! The edge over axum and hyper (architecture.md section 3).
//!
//! A [`Server`] wraps an [`edge::Edge`]: it accepts connections with hyper's HTTP/1 server, whose
//! header-read timeout and header buffer are the slow-client and oversized-head budgets of
//! [`edge::Limits`], hands each request to an axum router whose one fallback route is the edge,
//! streams the answer's blob a chunk at a time with idle and whole-response deadlines, and tells
//! an [`Observe`]r about every request as a typed [`Event`]. `/healthz` answers `200 ok` on a host
//! no site has, so a probe by address needs no name and no site loses a page of that name.
//!
//! The server owns the times; the edge owns what to answer. Stopping takes any future, so a test
//! needs no signal and the CLI passes Ctrl-C.

mod body;
mod bridge;
mod error;
mod event;
mod linger;
mod respond;
mod server;

pub use bridge::{Bridge, Local, Mount};
pub use error::Error;
pub use event::{Answered, Called, Cut, Event, Kind, Observe, Stopped};
pub use server::Server;
