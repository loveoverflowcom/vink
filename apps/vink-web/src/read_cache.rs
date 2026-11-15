//! Optional snapshot retention only. Request ownership, age and authority live outside storage.
use std::collections::BTreeMap;

pub trait SnapshotCache<K, V> {
    fn get(&self, key: &K) -> Option<V>;
    fn put(&mut self, key: K, value: V);
    fn remove(&mut self, key: &K);
    fn clear(&mut self);
}
#[derive(Clone, Debug)]
pub struct InMemoryCache<K, V>(BTreeMap<K, V>);
impl<K, V> Default for InMemoryCache<K, V> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}
impl<K: Ord, V: Clone> SnapshotCache<K, V> for InMemoryCache<K, V> {
    fn get(&self, key: &K) -> Option<V> {
        self.0.get(key).cloned()
    }
    fn put(&mut self, key: K, value: V) {
        self.0.insert(key, value);
    }
    fn remove(&mut self, key: &K) {
        self.0.remove(key);
    }
    fn clear(&mut self) {
        self.0.clear();
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
