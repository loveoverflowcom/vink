//! Optional snapshot retention only. Request ownership, age and authority live outside storage.
use std::collections::BTreeMap;

/// Explicit byte ceiling for optional in-tab `SnapshotCache` retention.
///
/// Weights track deterministic logical UTF-8 and structural container proxy bytes rather than
/// allocator RSS, browser process memory, or GPU buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetentionBudget {
    pub max_total_bytes: Option<usize>,
    pub max_entry_bytes: Option<usize>,
}

impl RetentionBudget {
    pub const UNBOUNDED: Self = Self {
        max_total_bytes: None,
        max_entry_bytes: None,
    };

    #[must_use]
    pub fn admits_weight(self, weight_bytes: usize) -> bool {
        self.max_entry_bytes.is_none_or(|max| weight_bytes <= max)
            && self.max_total_bytes.is_none_or(|max| weight_bytes <= max)
    }
}

pub trait SnapshotCache<K, V> {
    fn get(&self, key: &K) -> Option<V>;
    fn put(&mut self, key: K, value: V);
    fn remove(&mut self, key: &K);
    fn clear(&mut self);
}

#[derive(Clone, Debug)]
struct CachedEntry<V> {
    value: V,
    weight_bytes: usize,
    seq: u64,
}

#[derive(Clone, Debug)]
pub struct InMemoryCache<K, V> {
    entries: BTreeMap<K, CachedEntry<V>>,
    weigh: fn(&V) -> usize,
    budget: RetentionBudget,
    total_bytes: usize,
    next_seq: u64,
    evictions: u64,
    oversized_skips: u64,
}

impl<K, V> Default for InMemoryCache<K, V> {
    fn default() -> Self {
        Self::with_budget(RetentionBudget::UNBOUNDED, |_| 1)
    }
}

impl<K, V> InMemoryCache<K, V> {
    pub fn with_budget(budget: RetentionBudget, weigh: fn(&V) -> usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            weigh,
            budget,
            total_bytes: 0,
            next_seq: 0,
            evictions: 0,
            oversized_skips: 0,
        }
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn budget(&self) -> RetentionBudget {
        self.budget
    }

    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    pub fn oversized_skips(&self) -> u64 {
        self.oversized_skips
    }

    pub fn contains_key(&self, key: &K) -> bool
    where
        K: Ord,
    {
        self.entries.contains_key(key)
    }

    fn alloc_seq(&mut self) -> u64 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        seq
    }
}

impl<K: Ord + Clone, V: Clone> SnapshotCache<K, V> for InMemoryCache<K, V> {
    fn get(&self, key: &K) -> Option<V> {
        self.entries.get(key).map(|entry| entry.value.clone())
    }

    fn put(&mut self, key: K, value: V) {
        // Drop any prior weight for `key` first so updating an existing key to an oversized
        // snapshot never leaks or keeps stale retained data behind.
        self.remove(&key);
        let weight_bytes = (self.weigh)(&value);
        if !self.budget.admits_weight(weight_bytes) {
            self.oversized_skips = self.oversized_skips.saturating_add(1);
            return;
        }
        if let Some(max_total) = self.budget.max_total_bytes {
            while self.total_bytes.saturating_add(weight_bytes) > max_total {
                let Some(oldest_key) = self
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.seq)
                    .map(|(k, _)| k.clone())
                else {
                    break;
                };
                self.remove(&oldest_key);
                self.evictions = self.evictions.saturating_add(1);
            }
        }
        let seq = self.alloc_seq();
        self.total_bytes = self.total_bytes.saturating_add(weight_bytes);
        self.entries.insert(
            key,
            CachedEntry {
                value,
                weight_bytes,
                seq,
            },
        );
    }

    fn remove(&mut self, key: &K) {
        if let Some(removed) = self.entries.remove(key) {
            self.total_bytes = self.total_bytes.saturating_sub(removed.weight_bytes);
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.total_bytes = 0;
    }
}

#[derive(Clone, Debug, Default)]
pub struct NoOpCache;
impl<K, V> SnapshotCache<K, V> for NoOpCache {
    fn get(&self, _: &K) -> Option<V> {
        None
    }
    fn put(&mut self, _: K, _: V) {}
    fn remove(&mut self, _: &K) {}
    fn clear(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_cache_enforces_entry_and_total_ceilings_and_resets_bytes() {
        let budget = RetentionBudget {
            max_total_bytes: Some(100),
            max_entry_bytes: Some(60),
        };
        let mut cache: InMemoryCache<&'static str, Vec<u8>> =
            InMemoryCache::with_budget(budget, Vec::len);

        cache.put("a", vec![1; 40]);
        cache.put("b", vec![2; 45]);
        assert_eq!(cache.total_bytes(), 85);
        assert_eq!(cache.len(), 2);

        // Oversized single entry is skipped cleanly without disturbing existing entries.
        cache.put("too_big", vec![3; 61]);
        assert!(!cache.contains_key(&"too_big"));
        assert_eq!(cache.oversized_skips(), 1);
        assert_eq!(cache.total_bytes(), 85);

        // Overwriting an existing key with an oversized payload removes the old retained copy.
        cache.put("a", vec![4; 61]);
        assert!(!cache.contains_key(&"a"));
        assert_eq!(cache.oversized_skips(), 2);
        assert_eq!(cache.total_bytes(), 45);
        assert_eq!(cache.get(&"b"), Some(vec![2; 45]));

        // Admitting new keys evicts deterministically by oldest put sequence until within budget.
        cache.put("c", vec![5; 50]);
        assert_eq!(cache.total_bytes(), 95);
        assert_eq!(cache.evictions(), 0);
        cache.put("d", vec![6; 50]);
        // "b" (seq 1) must be evicted before "c" (seq 2) -> total = 50 + 50 = 100.
        assert!(!cache.contains_key(&"b"));
        assert!(cache.contains_key(&"c"));
        assert!(cache.contains_key(&"d"));
        assert_eq!(cache.total_bytes(), 100);
        assert_eq!(cache.evictions(), 1);

        cache.remove(&"c");
        assert_eq!(cache.total_bytes(), 50);
        cache.clear();
        assert_eq!(cache.total_bytes(), 0);
        assert!(cache.is_empty());
    }
}
