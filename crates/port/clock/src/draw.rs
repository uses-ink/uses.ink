//! Random bytes.

use std::convert::Infallible;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};

/// A source of the 16 bytes every id is made of.
pub trait Draw {
    /// Why the source gave none.
    type Error: Error + 'static;

    /// 16 bytes nobody has been given before.
    fn bytes(&self) -> Result<[u8; 16], Self::Error>;
}

impl<D: Draw + ?Sized> Draw for &D {
    type Error = D::Error;

    fn bytes(&self) -> Result<[u8; 16], Self::Error> {
        (**self).bytes()
    }
}

/// Ids that count: the first is 1, the next 2. The same calls in the same order draw the same
/// ids.
#[derive(Debug, Default)]
pub struct Counter(AtomicU64);

impl Counter {
    /// A counter that has drawn nothing.
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }
}

impl Draw for Counter {
    type Error = Infallible;

    fn bytes(&self) -> Result<[u8; 16], Infallible> {
        let next = self.0.fetch_add(1, Ordering::Relaxed).saturating_add(1);
        Ok(u128::from(next).to_be_bytes())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn counts_from_one() {
        let counter = Counter::new();
        assert_eq!(counter.bytes().ok(), Some(1_u128.to_be_bytes()));
        assert_eq!(counter.bytes().ok(), Some(2_u128.to_be_bytes()));
    }
}
