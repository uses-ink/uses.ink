//! Serving a deploy, independent of the HTTP runtime (architecture.md sections 3 and 11).
//!
//! An [`Edge`] turns a [`Request`] (a method, a target and header lines as bytes) into a typed
//! [`Reply`]: a [`Response`] and the [`Outcome`] that explains it. It resolves the host to a
//! deploy through [`Sites`], opens the blob behind a route through [`Blobs`], and refuses what it
//! should not serve as a [`Refusal`] with its data. It owns no socket, file, clock or runtime;
//! an adapter (`edge/axum`, later `edge/worker`) carries bytes in and out and enforces the times
//! of [`Limits`].
//!
//! Nothing here panics on any input. Every response, a refusal or a failure of ours included,
//! carries `nosniff`, a CSP and the cross-origin policy; a manifest entry's own headers (the
//! engine's page policy) win over those, and `Strict-Transport-Security` is added over TLS.

mod answer;
mod condition;
mod edge;
mod encoding;
mod entry;
mod error;
mod fields;
mod head;
mod host;
mod limits;
mod port;
mod range;
mod refusal;
mod request;
mod response;
mod tag;
mod target;

pub use answer::{Answer, Chain, Fault, Outcome, Ranging};
pub use condition::{ConditionError, Conditions};
pub use edge::{Caller, Edge, Hsts, Reply};
pub use encoding::{Accepted, Encoding};
pub use error::Error;
pub use fields::FieldError;
pub use head::{BodyError, HeaderError, Singular};
pub use host::{Authority, Hostname, HostnameError};
pub use limits::Limits;
pub use port::{Blob, BlobError, Blobs, Chunks, Deploy, Moved, Resolved, Sites, Variants};
pub use range::{Ignored, Pick, Ranges, Span};
pub use refusal::{Refusal, Stage};
pub use request::{Asked, Method, Request, Scheme};
pub use response::{Body, Field, Name, Response, Status};
pub use tag::{Matching, Strength, Tag, Validator};
pub use target::{Target, TargetError};
