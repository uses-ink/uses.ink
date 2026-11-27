//! The port itself.

use crate::{Binding, Error, Plain, Sealed};

/// Seals credentials and opens them again. The key lives in the adapter and nowhere else.
#[expect(async_fn_in_trait, reason = "a Workers secret binding's futures are not Send, so no Send bound can be promised")]
pub trait Secret {
    /// The adapter's own error.
    type Error: std::error::Error + 'static;

    /// Seals `plain` to `binding`. Sealing the same credential twice gives different bytes.
    async fn seal(&self, plain: &Plain, binding: &Binding) -> Result<Sealed, Error<Self::Error>>;

    /// Opens `sealed` for `binding`: [`Error::Refused`] if it was altered or is bound to another
    /// row.
    async fn open(&self, sealed: &Sealed, binding: &Binding) -> Result<Plain, Error<Self::Error>>;
}
