//! What the git-forge authorities share (decisions 0110 to 0113, `docs/architecture.md` section
//! 12). GitHub, Forgejo and GitLab each prove who owns a repository and each deliver pushes; this
//! crate is the part that is the same for all three, so the three crates differ only in the forge's
//! own spelling.
//!
//! - [`Repo`], [`Host`], [`Path`] and [`Segment`] are the typed names: a repository is a host and a
//!   lower-case path, never a string.
//! - [`Push`] is what a verified push comes to, and [`Ignored`] is why a verified delivery queues
//!   nothing. [`Roster`] is the set of repositories that sites follow.
//! - [`Intake`] is the order every webhook is checked in: size, then the forge's own signature, then
//!   the [`Replay`] window, then the body. [`Secret`] is the one type a credential lives in: it
//!   prints as `Secret(<redacted>)`, is zeroed on drop and compares in constant time.
//! - [`Transport`] is the port that reaches a forge's API; [`Fake`] answers from recorded replies and
//!   keeps what it was asked. [`Client`] stamps the configured limits on every call.
//! - [`Challenge`] is the repo challenge: a token committed to `.well-known/uses-ink`.
//! - [`Status`] is a commit status in the words every forge understands.

mod call;
mod error;
mod hook;
mod limits;
mod name;
mod secret;

pub use call::{
    Challenge, ChallengeError, Client, Fake, Faked, Method, Proven, Request, Response, State, Status, Transport,
    Unproven,
};
pub use error::{Fail, Refused};
pub use hook::{
    Commit, CommitError, Delivery, Headers, Ignored, Intake, Pinned, Push, Received, Replay, Replayed, Roster,
};
pub use limits::Limits;
pub use name::{Host, NameError, Path, Repo, Segment};
pub use secret::Secret;
