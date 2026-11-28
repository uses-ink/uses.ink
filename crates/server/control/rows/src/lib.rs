//! The floor the control plane's tables share (decision 0074): the typed ids, the names of the
//! tables and columns, the kinds of column a schema is made of, reading a stored row back into
//! values, the digest a secret is stored as, and the error for a column that does not read back.
//!
//! It knows the database port and the model and nothing of the control plane's domain, so the
//! domain rows, the sign-in subsystem and the keys subsystem each sit on it and none on another.

pub mod digest;
pub mod fields;
pub mod id;
#[allow(missing_docs, reason = "each variant is the table or column of its name")]
pub mod table;
pub mod typed;
pub mod unreadable;

pub use digest::{Digest, NotDigest};
pub use fields::Fields;
pub use id::{Account, Credential, Enrolment, Factor, Id, IdError, Kind, Session, Site};
pub use typed::Typed;
pub use unreadable::Unreadable;
