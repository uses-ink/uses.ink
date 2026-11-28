//! How git reaches an `https` remote (decision 0072).
//!
//! `gix` speaks https through `reqwest`, which here has no TLS feature of its own: [`Https`]
//! hands it one `rustls` client configuration, built over `ring` (the provider `lettre` already
//! brings) and the bundled web PKI roots. So there is one TLS stack in the tree, and nothing in
//! it reads the machine's certificate store, an environment variable or a git configuration file.
//!
//! What a connection may do is a typed setting: how long to wait to connect, how long one request
//! may take in all, and which redirects are followed. Certificates are always verified; there is
//! no setting that turns it off, and a certificate the roots do not vouch for fails the request
//! with the TLS error among its sources. A proxy is
//! never used: `gix`'s reqwest backend ignores `http.proxy`, and the client is built without a
//! system proxy.
//!
//! [`Fetch`] is the same client for an API that answers JSON or text (a forge's): one request, one
//! bounded answer, no redirect followed, the same roots and the same refusal of anything but https
//! (decision 0162).

mod error;
mod fetch;
mod follow;
mod https;

pub use error::Misconfigured;
pub use fetch::{Answer, Call, Failed, Fetch, Route, Verb};
pub use follow::Follow;
pub use https::Https;
