//! A flag that ends the calls watching it: the stages of one `farm check` share one, and the
//! stage that fails sets it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A cancellation that every clone sees. Setting it is final.
#[derive(Debug, Clone, Default)]
pub(crate) struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// Cancels every call watching this flag.
    pub(crate) fn set(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Whether it was set.
    pub(crate) fn is_set(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Two flags are equal when they are the same flag.
impl PartialEq for Cancel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Cancel {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_sees_the_flag_and_a_new_flag_does_not() {
        let flag = Cancel::default();
        let seen = flag.clone();
        assert!(!seen.is_set());
        flag.set();
        assert!(seen.is_set());
        assert_eq!(flag, seen);
        assert_ne!(flag, Cancel::default());
        assert!(!Cancel::default().is_set());
    }
}
