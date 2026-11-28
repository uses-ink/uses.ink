//! The Forgejo authority, `ink:authority-forgejo` (decision 0111, `docs/architecture.md` section
//! 12). Forgejo and Gitea share one webhook scheme, so this serves Codeberg and any instance an
//! operator lists.
//!
//! - [`Hooks`] verifies a delivery: `X-Forgejo-Signature` (or Gitea's spelling) is the hex
//!   HMAC-SHA256 of the raw body under the webhook's secret, compared in constant time, and a
//!   delivery id inside the replay window is refused.
//! - [`Forgejo::prove`] is the repo challenge: a token committed to `.well-known/uses-ink`, read
//!   with no credential. It is the baseline of section 12 and works on every instance with no OAuth
//!   application registered there; OAuth is the later, per-instance upgrade (decision 0111).
//! - [`Forgejo::status`] posts a commit status with a token that moves into the request.
//! - [`Instances`] is the list of instances this server calls, read once from `instances.toml`:
//!   Codeberg ships pre-registered, and a host that is not listed is never called.

mod forgejo;
mod hooks;
mod instance;

pub use forgejo::Forgejo;
pub use hooks::Hooks;
pub use instance::{Instance, Instances, Unreadable};
