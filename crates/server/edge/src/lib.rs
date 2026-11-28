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

mod http;
mod serve;

pub use http::answer::{Answer, Fault, Outcome, Ranging};
pub use http::condition::{ConditionError, Conditions};
pub use http::encoding::{Accepted, Encoding};
pub use http::etag::{Matching, Strength, Tag, Validator};
pub use http::fields::FieldError;
pub use http::head::{BodyError, HeaderError, Singular};
pub use http::limits::Limits;
pub use http::range::{Ignored, Pick, Ranges, Span};
pub use http::refusal::{Refusal, Stage};
pub use http::request::{Asked, Method, Request, Scheme};
pub use http::response::{Body, Field, Name, Response, Status};
pub use model::Chain;
pub use serve::error::Error;
pub use serve::hostname::{Authority, Hostname, HostnameError};
pub use serve::port::{Blob, BlobError, Blobs, Chunks, Deploy, Moved, Notice, Resolved, Sites, Variants};
pub use serve::target::{Target, TargetError};
pub use serve::{Caller, Edge, Hsts, Reply};
