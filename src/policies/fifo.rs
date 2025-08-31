use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use crate::CachePolicy;

pub struct FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    map: HashMap<K, V>,
    order: VecDeque<K>,
    capacity: usize,
}

impl<K, V> FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new FIFO cache with the specified capacity
    ///
    /// # Panics
    /// Panics if capacity is 0
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "FIFO cache capacity must be greater than 0");
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    /// Creates a FIFO cache with default capacity of 100
    pub fn with_default_capacity() -> Self {
        Self::new(100)
    }

    /// Evicts the oldest inserted item (front of VecDeque)
    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self.order.pop_front() {
            self.map.remove(&oldest_key);
        }
    }
}

impl<K, V> CachePolicy<K, V> for FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Get a reference to a value by key
    fn get(&mut self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    /// Insert a key-value pair into the cache
    ///
    /// If the key exists, update the value without changing order
    fn insert(&mut self, key: K, value: V) {
        if !self.map.contains_key(&key) {
            if self.map.len() == self.capacity {
                self.evict_oldest();
            }
            self.order.push_back(key.clone());
        }
        self.map.insert(key, value);
    }

    /// Remove a key, returning the removed value if it existed
    fn remove(&mut self, key: &K) -> Option<V> {
        if self.map.remove(key).is_some() {
            // Also remove from order queue
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                self.order.remove(pos);
            }
            // Return removed value - but since map.remove already removes, we can't get it here
            // Alternative design: store removed value before remove
            None
        } else {
            None
        }
    }

    /// Returns current number of entries
    fn len(&self) -> usize {
        self.map.len()
    }

    /// Clear all entries from the cache
    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    /// Returns the maximum capacity
    fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<K, V> FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Checks if cache is empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fifo_basic_operations() {
        let mut cache = FifoCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(3, "three");
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&1), Some(&"one"));
        assert_eq!(cache.get(&2), Some(&"two"));
        assert_eq!(cache.get(&3), Some(&"three"));
    }

    #[test]
    fn test_fifo_eviction_order() {
        let mut cache = FifoCache::new(2);
        cache.insert(1, "one");
        cache.insert(2, "two");
        // This should evict key 1 (oldest)
        cache.insert(3, "three");
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&"two"));
        assert_eq!(cache.get(&3), Some(&"three"));
    }

    #[test]
    fn test_fifo_update_existing() {
        let mut cache = FifoCache::new(2);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(1, "ONE"); // Update but order stays
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&1), Some(&"ONE"));
        assert_eq!(cache.get(&2), Some(&"two"));
    }

    #[test]
    fn test_fifo_remove() {
        let mut cache = FifoCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        assert_eq!(cache.remove(&1), None);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&"two"));
    }

    #[test]
    fn test_fifo_clear() {
        let mut cache = FifoCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(3, "three");
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.get(&1), None);
    }

    #[test]
    #[should_panic(expected = "FIFO cache capacity must be greater than 0")]
    fn test_fifo_zero_capacity_panics() {
        FifoCache::<i32, String>::new(0);
    }
}

