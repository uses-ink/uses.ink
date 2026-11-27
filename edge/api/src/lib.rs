//! The edge's API and intake (architecture section 3, decision 0052).
//!
//! An [`Api`] turns a [`Request`] (a method, a path, an `Authorization` value and a bounded
//! body) into a [`Reply`], over a [`control::Control`] and three more ports: [`Credentials`] says
//! who a bearer token speaks for, [`clock::Clock`] says what time it is and [`clock::Draw`] gives
//! the bytes of new ids. It owns no socket and no runtime; `edge/axum` carries bytes in and out.
//!
//! - **Routes** are one enum, [`Call`], parsed from a method and a path with every id checked.
//!   A path that is no route, or a method a route does not take, says so with its data.
//! - **Requests and replies** are typed values: [`body`] reads what a route takes, [`view`]
//!   holds what it gives, and every failure is a [`Fault`] that becomes a [`view::Problem`].
//! - **Sites are the caller's**: a site of another account is answered as no site at all.
//! - **Intake** takes a tar archive of what `ink build` wrote, reads it within [`Limits`] (decision
//!   0044: the operator's flags are ceilings and a site may only lower them, see
//!   [`Limits::lower`]), checks the manifest and every file's hash, and records a deploy.
//!
//! The one credential adapter here is [`Owner`], a single token for a self-host with one person.

mod api;
pub mod body;
mod credential;
mod export;
mod fault;
mod intake;
mod limits;
mod reply;
mod route;
pub mod tar;
pub mod view;

pub use api::{Answers, Api};
pub use credential::{Bearer, BearerError, Credentials, LONGEST, Owner, Principal, Token};
pub use export::{Exports, Route};
pub use fault::{Denied, Fault, Intake};
pub use limits::{Asked, Limit, Limits, Raised};
pub use reply::{Media, Method, Payload, Reply, Request, Status, UnknownMethod};
pub use route::{Call, RouteError};
