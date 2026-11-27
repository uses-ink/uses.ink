//! The tar a folder travels in: a reader that trusts nothing and a writer that makes only what
//! the reader takes.
//!
//! Only regular files and folders are read. A link, a device, a pax header, a name that is no
//! [`ContentPath`](model::ContentPath), a checksum that does not hold and a size that is more
//! than the limits allow are each a typed refusal that says where in the archive it is. Sizes are
//! counted as the archive is read, never taken from a header on trust. Names longer than a
//! ustar header holds use the GNU long-name entry on both sides.

mod error;
mod read;
mod write;

pub use error::Error;
pub use read::{Limits, Tree};
pub use write::Writer;

/// The size of a tar block, and of a header.
pub(crate) const BLOCK: usize = 512;
