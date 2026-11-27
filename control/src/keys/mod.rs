//! Keys for machines (decision 0068, `docs/auth.md` sections 5 to 7, 12 and 13): the rights a
//! credential has, the device and deploy keys an account holds, enrolling them, and the
//! lifecycle events they leave.
//!
//! - **No secret signs in.** A key row holds a public key; the private half never reaches the
//!   server. The profile and the key types are the `sig` crate's.
//! - **Rights** are typed: [`Right`], [`Scope`], [`Grant`], composed by union with no deny
//!   ([`Grants`]), and a device key is an attenuation of the person's ([`Effective`]). The
//!   defaults are least privilege ([`Rights::DEVICE`], [`Rights::DEPLOY`]).
//! - **Enrolment** is the device flow with the key pair as the secret ([`Enrolment`],
//!   [`UserCode`]); [`Keys::approve`] is what a person's approval does, behind whatever proves
//!   the person.
//! - **Audit** is lifecycle events ([`Audit`]) and a date on the key's row (`last_used`), never a
//!   per-request log.
//!
//! The tables are made by [`Entry`], a migration to apply after the sign-in's.

mod audit;
mod enrol;
mod error;
mod key;
mod migration;
mod right;
mod store;

pub use audit::{Audit, Entry as Event};
pub use enrol::{Enrolment, NewEnrolment, Poll, Seen, State as EnrolmentState, UserCode, UserCodeError};
pub use error::{Error, Refused};
pub use key::{Key, Kind, Label, LabelError, State, Why};
pub use migration::Entry;
pub use right::{Effective, Grant, Grants, Right, Rights, Scope, Target, UnknownRight};
pub use store::{Approval, Keys, Settings};
