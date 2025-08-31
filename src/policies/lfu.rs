use std::collections::{HashMap, BTreeMap};
use std::hash::Hash;
use crate::CachePolicy;

/// A Least Frequently Used (LFU) cache implementation
///
/// This cache evicts the item with the lowest access frequency.
/// When multiple keys have the same frequency, the oldest inserted among them is evicted.
pub struct LfuCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Maps key to (value, frequency)
    map: HashMap<K, (V, usize)>,
    /// Maps frequency to keys inserted in this frequency in order (to get oldest for eviction)
    freq_list: BTreeMap<usize, Vec<K>>,
    /// Maximum capacity of the cache
    capacity: usize,
    /// Tracks the minimum frequency currently in the cache for quick eviction
    min_freq: usize,
}

impl<K, V> LfuCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new LFU cache with specified capacity
    ///
    /// # Panics
    /// Panics if capacity is 0
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "LFU cache capacity must be greater than 0");
        Self {
            map: HashMap::new(),
            freq_list: BTreeMap::new(),
            capacity,
            min_freq: 0,
        }
    }

    /// Creates an LFU cache with default capacity 100
    pub fn with_default_capacity() -> Self {
        Self::new(100)
    }

    /// Helper to increment frequency of a key accessed
    fn increase_freq(&mut self, key: &K) {
        if let Some((_, freq)) = self.map.get_mut(key) {
            // Remove key from old frequency list
            if let Some(keys) = self.freq_list.get_mut(freq) {
                if let Some(pos) = keys.iter().position(|k| k == key) {
                    keys.swap_remove(pos);
                }
            }
            // Clean old freq entry if empty
            if let Some(keys) = self.freq_list.get(freq) {
                if keys.is_empty() {
                    self.freq_list.remove(freq);
                    if *freq == self.min_freq {
                        self.min_freq += 1;
                    }
                }
            }

            // Increase frequency by 1
            *freq += 1;
            // Add key to new frequency list
            self.freq_list.entry(*freq).or_default().push(key.clone());
        }
    }

    /// Evicts one key with the lowest frequency (min_freq)
    ///
    /// Chooses the oldest inserted key among those with minimal frequency.
    fn evict(&mut self) {
        if let Some(keys) = self.freq_list.get_mut(&self.min_freq) {
            if let Some(oldest_key) = keys.first().cloned() {
                // Remove from freq_list and map
                keys.remove(0);
                if keys.is_empty() {
                    self.freq_list.remove(&self.min_freq);
                }
                self.map.remove(&oldest_key);
            }
        }
    }
}

impl<K, V> CachePolicy<K, V> for LfuCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Gets value by key and increases its frequency
    fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            self.increase_freq(key);
            self.map.get(key).map(|v| &v.0)
        } else {
            None
        }
    }

    /// Inserts or updates a key-value pair
    ///
    /// Evicts least frequently used when capacity exceeded.
    fn insert(&mut self, key: K, value: V) {
        if self.capacity == 0 {
            return;
        }

        if self.map.contains_key(&key) {
            // Update value and increase frequency
            if let Some((v, _)) = self.map.get_mut(&key) {
                *v = value;
            }
            self.increase_freq(&key);
            return;
        }

        if self.map.len() == self.capacity {
            self.evict();
        }

        // Insert with freq 1
        self.map.insert(key.clone(), (value, 1));
        self.freq_list.entry(1).or_default().push(key);
        self.min_freq = 1; // Reset min_freq as new key added with freq 1
    }

    /// Removes a key, returning its value if present
    fn remove(&mut self, key: &K) -> Option<V> {
        if let Some((value, freq)) = self.map.remove(key) {
            if let Some(keys) = self.freq_list.get_mut(&freq) {
                if let Some(pos) = keys.iter().position(|k| k == key) {
                    keys.remove(pos);
                }
                if keys.is_empty() {
                    self.freq_list.remove(&freq);
                }
            }
            Some(value)
        } else {
            None
        }
    }

    /// Returns number of items currently stored
    fn len(&self) -> usize {
        self.map.len()
    }

    /// Removes all entries from the cache
    fn clear(&mut self) {
        self.map.clear();
        self.freq_list.clear();
        self.min_freq = 0;
    }

    /// Returns maximal capacity allowed
    fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Safe cleanup via drop
impl<K, V> Drop for LfuCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

// Implement Send and Sync if K and V satisfy bounds
unsafe impl<K, V> Send for LfuCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}
unsafe impl<K, V> Sync for LfuCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfu_basic_operations() {
        let mut cache = LfuCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(3, "three");
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&1), Some(&"one"));
        assert_eq!(cache.get(&2), Some(&"two"));
        assert_eq!(cache.get(&3), Some(&"three"));
    }

    #[test]
    fn test_lfu_eviction_order() {
        let mut cache = LfuCache::new(2);
        cache.insert(1, "one");
        cache.insert(2, "two");

        // Access key 1 once (freq now 2)
        cache.get(&1);

        // Insert 3, should evict key 2 (freq=1, least)
        cache.insert(3, "three");
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&1), Some(&"one"));
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&"three"));
    }

    #[test]
    fn test_lfu_update_value() {
        let mut cache = LfuCache::new(2);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(1, "ONE"); // update existing key 1
        assert_eq!(cache.get(&1), Some(&"ONE"));
        assert_eq!(cache.get(&2), Some(&"two"));
    }

    #[test]
    fn test_lfu_remove() {
        let mut cache = LfuCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        assert_eq!(cache.remove(&1), Some("one"));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&"two"));
    }

    #[test]
    fn test_lfu_clear() {
        let mut cache = LfuCache::new(3);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get(&1).is_none());
    }

    #[test]
    #[should_panic(expected = "LFU cache capacity must be greater than 0")]
    fn test_zero_capacity_panics() {
        LfuCache::<i32, String>::new(0);
    }
}

