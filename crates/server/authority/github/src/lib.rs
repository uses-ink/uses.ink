//! The GitHub authority, `ink:authority-github` (decision 0110, `docs/architecture.md` section 12,
//! `docs/hosted.md` B3 and HQ13): one GitHub App with the least it needs.
//!
//! - [`Hooks`] verifies a webhook delivery: `X-Hub-Signature-256` is the HMAC-SHA256 of the raw
//!   body under the App's webhook secret, compared in constant time, and a delivery id inside the
//!   replay window is refused. What survives is a typed [`authority_forge::Received`].
//! - [`App`] speaks as the App. [`App::owns`] proves that the App is installed on a repository by
//!   the account that claimed it. [`App::token`] exchanges the App's [`Jwt`] for an installation
//!   token scoped to one repository. [`App::status`] posts a commit status with a token it
//!   exchanged for that call. Nothing holds a token: it is a [`authority_forge::Secret`], which
//!   prints as `Secret(<redacted>)` and is zeroed when dropped.
//! - [`Signer`] signs RS256; [`Key`] is the one over `ring` that reads the PEM GitHub issues.
//!   Every call goes through [`authority_forge::Transport`], which has a fake, so none of this
//!   reaches GitHub in its tests.

mod app;
mod error;
mod hooks;
mod id;
mod jwt;
mod key;

pub use app::{App, Covered, Grant, Target};
pub use error::Error;
pub use hooks::Hooks;
pub use id::{AppId, InstallationId};
pub use jwt::{Jwt, Unsigned};
pub use key::{Armor, Key, KeyError, Signer};
