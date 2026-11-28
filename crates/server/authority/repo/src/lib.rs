//! The repository authority (decision 0160): a site claims its source repository, commits the
//! challenge it was given, and the repository then builds the site when a forge's webhook says it
//! moved.
//!
//! - [`Repos`] reads the challenge file through the forge a repository lives on (GitHub with no
//!   credential, an operator's list of Forgejo instances, GitLab hosts), and hands the control plane
//!   what it found. It claims, proves, rotates the webhook secret and releases.
//! - [`Shared`] is a transport behind a pointer, so the three forges call through one.
//! - [`Wire`] (the `system` feature) is the transport that reaches a forge over https: a host
//!   allowlist, a reply limit, no redirect, a timeout, and the forge's name sent to the address the
//!   operator gave when there is one.

mod error;
mod repos;
mod shared;
#[cfg(feature = "system")]
mod wire;

pub use error::Error;
pub use repos::{Claimed, Forge, Forges, Repos, Unknown};
pub use shared::Shared;
pub use shared::{Silent, Unreached};
#[cfg(feature = "system")]
pub use wire::{Unwired, Wire};
