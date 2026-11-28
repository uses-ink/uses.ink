//! The GitLab authority, `ink:authority-gitlab` (decision 0112, `docs/architecture.md` section 12).
//!
//! - [`Hooks`] verifies a delivery. GitLab does not sign the body: it sends the secret token as the
//!   `X-Gitlab-Token` header, so the check is a constant-time compare of that token (over digests,
//!   so its length does not leak either) and a delivery id inside the replay window is refused.
//!   What survives is a typed [`authority_forge::Received`].
//! - [`Gitlab::prove`] is the repo challenge: a token committed to `.well-known/uses-ink` on the
//!   default branch, read with no credential through the repository files API. Subgroups work: the
//!   path `group/sub/name` is one encoded segment, as GitLab takes it.
//! - [`Gitlab::status`] posts a commit status with a token that moves into the request.

mod gitlab;
mod hooks;

pub use gitlab::Gitlab;
pub use hooks::Hooks;
