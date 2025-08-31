use std::collections::HashMap;
use std::hash::Hash;
use rand::{thread_rng, Rng};
use crate::CachePolicy;

/// A cache policy that evicts a random entry when capacity is exceeded.
///
/// This implementation uses a HashMap for storage and selects keys randomly for eviction.
pub struct RandomCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    map: HashMap<K, V>,
    capacity: usize,
}

impl<K, V> RandomCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new RandomCache with the specified capacity
    ///
    /// # Panics
    /// Panics if capacity is 0
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Random cache capacity must be greater than 0");
        Self {
            map: HashMap::new(),
            capacity,
        }
    }

    /// Creates a RandomCache with default capacity of 100
    pub fn with_default_capacity() -> Self {
        Self::new(100)
    }

    /// Evicts a random key from the cache
    fn evict_random(&mut self) {
        if self.map.is_empty() {
            return;
        }
        let mut rng = thread_rng();
        // Collect keys and pick a random index
        let keys: Vec<K> = self.map.keys().cloned().collect();
        if let Some(random_key) = keys.get(rng.gen_range(0..keys.len())) {
            self.map.remove(random_key);
        }
    }
}

impl<K, V> CachePolicy<K, V> for RandomCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Get a reference to the value for the given key without affecting eviction order
    fn get(&mut self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    /// Insert or update a key-value pair
    ///
    /// Evicts a random entry if capacity is exceeded.
    fn insert(&mut self, key: K, value: V) {
        if !self.map.contains_key(&key) && self.map.len() == self.capacity {
            self.evict_random();
        }
        self.map.insert(key, value);
    }

    /// Remove an entry by key, returning the value if it existed
    fn remove(&mut self, key: &K) -> Option<V> {
        self.map.remove(key)
    }

    /// Returns the current number of entries in the cache
    fn len(&self) -> usize {
        self.map.len()
    }

    /// Removes all entries from the cache
    fn clear(&mut self) {
        self.map.clear();
    }

    /// Returns the maximum capacity of the cache
    fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Safe cleanup via Drop trait
impl<K, V> Drop for RandomCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

// Implement Send and Sync if K and V satisfy bounds
unsafe impl<K, V> Send for RandomCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}
unsafe impl<K, V> Sync for RandomCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_basic_operations() {
        let mut cache = RandomCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(3, "three");
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&1), Some(&"one"));
        assert_eq!(cache.get(&2), Some(&"two"));
        assert_eq!(cache.get(&3), Some(&"three"));
    }

    #[test]
    fn test_random_eviction() {
        let mut cache = RandomCache::new(2);
        cache.insert(1, "one");
        cache.insert(2, "two");
        // Insert third, triggers random eviction
        cache.insert(3, "three");
        assert_eq!(cache.len(), 2);
        // Exactly which key got evicted is non-deterministic, so test that at least one original key is gone
        let one_present = cache.get(&1).is_some();
        let two_present = cache.get(&2).is_some();
        let three_present = cache.get(&3).is_some();
        assert!(one_present || two_present); // One of first two keys remains
        assert!(three_present); // New key should be present
    }

    #[test]
    fn test_random_update_existing() {
        let mut cache = RandomCache::new(2);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(1, "ONE"); // Update existing key 1
        assert_eq!(cache.get(&1), Some(&"ONE"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_random_remove() {
        let mut cache = RandomCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        assert_eq!(cache.remove(&1), Some("one"));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&"two"));
    }

    #[test]
    fn test_random_clear() {
        let mut cache = RandomCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get(&1).is_none());
    }

    #[test]
    #[should_panic(expected = "Random cache capacity must be greater than 0")]
    fn test_random_zero_capacity_panics() {
        RandomCache::<i32, String>::new(0);
    }
}

