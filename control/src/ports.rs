//! The ports the control plane runs on.

use blob::Blob;
use db::Db;
use queue::Queue;
use secret::Secret;

/// The four ports a [`crate::Control`] is made of. The build port is not here: it is handed to
/// the one use case that runs builds, so a control plane that only serves reads needs none.
///
/// A tuple of the four is the usual implementation: `(db, blob, queue, secret)`.
pub trait Ports {
    /// The rows.
    type Db: Db;
    /// The blobs.
    type Blob: Blob;
    /// The build requests.
    type Queue: Queue;
    /// The sealed credentials.
    type Secret: Secret;

    /// The database.
    fn db(&self) -> &Self::Db;
    /// The blob store.
    fn blob(&self) -> &Self::Blob;
    /// The queue.
    fn queue(&self) -> &Self::Queue;
    /// The sealer.
    fn secret(&self) -> &Self::Secret;
}

impl<D: Db, B: Blob, Q: Queue, S: Secret> Ports for (D, B, Q, S) {
    type Db = D;
    type Blob = B;
    type Queue = Q;
    type Secret = S;

    fn db(&self) -> &D {
        &self.0
    }

    fn blob(&self) -> &B {
        &self.1
    }

    fn queue(&self) -> &Q {
        &self.2
    }

    fn secret(&self) -> &S {
        &self.3
    }
}
