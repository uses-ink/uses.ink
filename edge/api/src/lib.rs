//! The edge's API and intake (architecture section 3, decision 0052).
//!
//! An [`Api`] turns a [`Request`] (a method, a path, an `Authorization` value and a bounded
//! body) into a [`Reply`], over a [`control::Control`] and three more ports: [`Credentials`] says
//! who a signature speaks for, [`clock::Clock`] says what time it is and [`clock::Draw`] gives
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
//! **Who calls** is a signature (decision 0068): every call but `POST /v1/enrol` carries an
//! `Authorization: Ink-Sig-1` header that [`Signatures`] checks over the [`Credentials`] port,
//! and the API reads no cookie. The one operator of a self-host has keys in a file
//! ([`OperatorFile`]); everyone else's are rows, enrolled by a device-code flow that a person
//! approves at the [`Door`] through the [`Approver`] port.

mod api;
pub mod body;
mod cookie;
mod credential;
pub mod door;
mod enrol;
mod export;
mod fault;
mod intake;
mod limits;
mod operator;
mod refusal;
mod reply;
mod route;
mod signature;
pub mod tar;
pub mod view;

pub use api::{Answers, Api};
pub use cookie::{Cookie, Jar, JarError, Name, NameError};
pub use credential::{Credentials, Offered, Presented, Principal, Verdict, Via};
pub use door::{
    Approver, Approving, Config, Courier, Door, Draws, Enter, Limiter, Note, Notes, Outbox, Rate, Rates, Routed,
    Strength,
};
pub use enrol::{Closed, Closing, Enrol, Gate};
pub use export::{Exports, Route};
pub use fault::{Denied, Fault, Intake};
pub use limits::{Asked, Limit, Limits, Raised};
pub use operator::{NoOperators, OperatorFile, OperatorKey, Operators, Unreadable as OperatorError};
pub use reply::{Media, Method, Payload, Reply, Request, Status, UnknownMethod};
pub use route::{Call, RouteError};
pub use signature::{Claimed, Config as Signing, Failed, Nonces, Signatures, Window, WindowError};
