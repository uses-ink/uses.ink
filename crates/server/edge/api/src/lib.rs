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
pub mod archive;
mod auth;
pub mod body;
mod ceremony;
mod enrol;
mod fault;
mod hooks;
mod limits;
mod nonce;
mod operator;
mod port;
mod refusal;
mod reply;
mod route;
mod signature;
mod sites;
mod source;
pub mod view;
mod webauthn;

pub use api::{Answers, Api};
/// The old path of [`archive`], kept for the crates that import it.
pub use archive as tar;
pub use auth::cookie::{Cookie, Jar, JarError, Name, NameError};
pub use auth::credential::{Credentials, Offered, Presented, Principal, Verdict, Via};
pub use auth::origin::Origins;
/// The old path of [`auth::signin`], kept for the crates that import it.
pub use auth::signin as door;
pub use auth::signin::{
    Approver, Approving, Config, Courier, Door, Draws, Enter, Limiter, Note, Notes, Outbox, Owner, Quota, Rate, Rates,
    Routed, Strength,
};
pub use ceremony::{Ceremonies, Passing, Step, Stop};
pub use enrol::{Closed, Closing, Enrol, Gate};
pub use fault::{Denied, Fault, Forged, Intake};
pub use hooks::{HOOKS, Hooks, Told};
pub use limits::{Asked, Limit, Limits, Raised};
pub use nonce::Case as NonceCase;
pub use nonce::{Claimed, Claiming, Nonces, Remembered};
pub use operator::{NoOperators, OperatorFile, OperatorKey, Operators, Roll, Unreadable as OperatorError};
pub use port::{Port, Unnamed};
pub use reply::{Header, Media, Method, Payload, Reply, Request, Status, UnknownMethod};
pub use route::{Call, RepoStep, RouteError};
pub use signature::{Config as Signing, Failed, Signatures, Window, WindowError};
pub use sites::{Exports, Route};
pub use source::{Probed, Sourced, Stored};
pub use webauthn::{Failed as Unavailable, Webauthn};
