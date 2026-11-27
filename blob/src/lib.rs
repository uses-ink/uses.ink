//! The blob port of the control plane (architecture section 11): any byte string at
//! `blake3(bytes)`, under a [`Tenant`]. Scoped per tenant, so there is no cross-tenant dedup,
//! deletion is exact, and one tenant cannot learn what another holds.
//!
//! Keys are only ever a [`Tenant`] and a [`model::Hash`], both validated types, so an adapter
//! builds no path or key from text an outsider chose. A write names the hash it expects and the
//! adapter refuses bytes that do not match ([`Error::Mismatch`]); a read verifies as it streams
//! ([`Error::Corrupt`]).
//!
//! The adapters are `blob/dir` and, hosted, `blob/r2`; the suite every adapter runs is
//! `blob/conformance`.

mod error;
mod port;
mod tenant;

pub use error::Error;
pub use port::{Blob, Chunks, Deleted, Listed, Stored};
pub use tenant::{Tenant, TenantError};
