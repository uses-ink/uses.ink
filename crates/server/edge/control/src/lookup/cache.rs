//! What the lookup may keep and for how long: its settings and the bounded memory they size.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

/// How the lookup behaves. Every value is the operator's to set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// How long a handle's live deploy is remembered: a deploy that went live or a rollback is
    /// seen within this long. Zero asks the control plane every time.
    pub ttl: Duration,
    /// How many decoded manifests are kept. Zero keeps none.
    pub deploys: usize,
    /// How many lookups may wait for the control plane before a caller waits for room.
    pub waiting: usize,
}

impl Settings {
    /// Ten seconds: what architecture section 11 says of the lookup per colo.
    pub const TTL: Duration = Duration::from_secs(10);
}

impl Default for Settings {
    fn default() -> Self {
        Self { ttl: Self::TTL, deploys: 64, waiting: 64 }
    }
}

/// At most `capacity` values by key; adding one past that forgets the oldest added.
#[derive(Debug)]
pub(crate) struct Recent<K, V> {
    capacity: usize,
    order: VecDeque<K>,
    items: BTreeMap<K, V>,
}

impl<K: Ord + Clone, V> Recent<K, V> {
    pub(crate) const fn new(capacity: usize) -> Self {
        Self { capacity, order: VecDeque::new(), items: BTreeMap::new() }
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.items.get(key)
    }

    pub(crate) fn put(&mut self, key: K, value: V) {
        if self.capacity == 0 {
            return;
        }
        if self.items.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
        while self.items.len() > self.capacity {
            match self.order.pop_front() {
                Some(oldest) => self.items.remove(&oldest),
                None => break,
            };
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn forgets_the_oldest_past_its_capacity() {
        let mut recent = Recent::new(2);
        recent.put(1, "a");
        recent.put(2, "b");
        recent.put(3, "c");
        assert_eq!((recent.get(&1), recent.get(&2), recent.get(&3)), (None, Some(&"b"), Some(&"c")));
        let mut none = Recent::new(0);
        none.put(1, "a");
        assert_eq!(none.get(&1), None);
    }
}
