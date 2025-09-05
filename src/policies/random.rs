use std::collections::HashMap;
use std::hash::Hash;
use rand::{thread_rng, Rng};
use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::NoPrefetch;

/// A cache policy that randomly evicts entries when the cache reaches its capacity.
/// 
/// This cache stores values in a HashMap for fast lookup, and when an insertion would cause
/// the cache to exceed its maximum size, a random key is evicted to make space.
/// 
/// Additionally, this cache supports prefetch strategies similar to FIFO cache,
/// using a prefetch buffer and tracking prefetch statistics to predict and proactively
/// load potential next keys.
pub struct RandomCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Main cache storage mapping keys to values.
    map: HashMap<K, V>,

    /// Maximum capacity of the cache.
    capacity: usize,

    /// Prefetch strategy used to predict future key accesses.
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,

    /// Buffer holding prefetched entries not yet accessed by `get`.
    prefetch_buffer: HashMap<K, V>,

    /// Maximum size of the prefetch buffer.
    prefetch_buffer_size: usize,

    /// Statistics tracking prefetch predictions and effectiveness.
    prefetch_stats: PrefetchStats,
}

/// Stores statistical data about prefetch operation efficiency.
#[derive(Debug, Clone, Default)]
pub struct PrefetchStats {
    /// Number of prefetch predictions made.
    pub predictions_made: u64,

    /// Number of times a prefetched key was actually accessed.
    pub prefetch_hits: u64,

    /// Number of times a prefetched key was not used.
    pub prefetch_misses: u64,

    /// Number of cache hits satisfied directly from prefetched entries.
    pub cache_hits_from_prefetch: u64,
}

impl PrefetchStats {
    /// Calculate the percentage of prefetch predictions that were hits.
    pub fn hit_rate(&self) -> f64 {
        if self.predictions_made == 0 {
            0.0
        } else {
            (self.prefetch_hits as f64 / self.predictions_made as f64) * 100.0
        }
    }

    /// Calculate the effectiveness of prefetching in serving cache hits.
    pub fn effectiveness(&self) -> f64 {
        if self.prefetch_hits == 0 {
            0.0
        } else {
            (self.cache_hits_from_prefetch as f64 / self.prefetch_hits as f64) * 100.0
        }
    }
}

impl<K, V> RandomCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new `RandomCache` with the specified capacity using no prefetch strategy.
    ///
    /// # Panics
    /// Panics if the given capacity is 0.
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Create a new `RandomCache` with a custom prefetch strategy.
    ///
    /// # Panics
    /// Panics if the given capacity is 0.
    pub fn with_custom_prefetch(
        capacity: usize,
        prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    ) -> Self {
        assert!(capacity > 0, "Random cache capacity must be greater than 0");

        Self {
            map: HashMap::new(),
            capacity,
            prefetch_strategy,
            prefetch_buffer: HashMap::new(),
            prefetch_buffer_size: (capacity / 4).max(1),
            prefetch_stats: PrefetchStats::default(),
        }
    }

    /// Create a new `RandomCache` with a default capacity of 100 entries.
    pub fn with_default_capacity() -> Self {
        Self::new(100)
    }

    /// Set the maximum size of the prefetch buffer.
    pub fn set_prefetch_buffer_size(&mut self, size: usize) {
        self.prefetch_buffer_size = size.max(1);
        self.trim_prefetch_buffer();
    }

    /// Get a reference to the current prefetch statistics.
    pub fn prefetch_stats(&self) -> &PrefetchStats {
        &self.prefetch_stats
    }

    /// Reset all prefetch statistics and the prefetch strategy state.
    pub fn reset_prefetch_stats(&mut self) {
        self.prefetch_stats = PrefetchStats::default();
        self.prefetch_strategy.reset();
    }

    /// Remove old entries from the prefetch buffer while exceeding buffer size.
    fn trim_prefetch_buffer(&mut self) {
        while self.prefetch_buffer.len() > self.prefetch_buffer_size {
            if let Some(key) = self.prefetch_buffer.keys().next().cloned() {
                self.prefetch_buffer.remove(&key);
            } else {
                break;
            }
        }
    }

    /// Perform prefetch predictions based on the accessed key and update the prefetch buffer.
    ///
    /// This calls the prefetch strategy to get predicted next keys and records predictions.
    /// Actual loading of predicted values from a data source is not performed here.
    fn perform_prefetch(&mut self, accessed_key: &K) {
        self.prefetch_strategy.update_access_pattern(accessed_key);
        let predictions = self.prefetch_strategy.predict_next(accessed_key);

        for predicted_key in predictions {
            self.prefetch_stats.predictions_made += 1;

            // Only prefetch keys that are not already cached or prefetched.
            if !self.map.contains_key(&predicted_key)
                && !self.prefetch_buffer.contains_key(&predicted_key)
            {
                // Prefetch loading would happen here if implemented.
            }
        }

        self.trim_prefetch_buffer();
    }

    /// Evict a random entry from the main cache to free space.
    fn evict_random(&mut self) {
        if self.map.is_empty() {
            return;
        }
        let mut rng = thread_rng();
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
    /// Retrieve a reference to a value by key.
    ///
    /// - If the key is found in the prefetch buffer, move it into the main cache and return it.
    /// - Otherwise, check the main cache; if present, perform prefetching before returning.
    fn get(&mut self, key: &K) -> Option<&V> {
        // Check prefetch buffer first
        if let Some(_) = self.prefetch_buffer.get(key) {
            if let Some(value) = self.prefetch_buffer.remove(key) {
                self.prefetch_stats.cache_hits_from_prefetch += 1;
                self.insert(key.clone(), value);
                return self.get(key);
            }
        }

        // Check main cache presence, perform prefetch, then return reference.
        if self.map.contains_key(key) {
            self.perform_prefetch(key);
            self.map.get(key)
        } else {
            None
        }
    }

    /// Insert or update a key-value pair.
    ///
    /// Evicts a random entry if the cache is full and the key is new.
    /// Removes any existing entry for the key from the prefetch buffer.
    fn insert(&mut self, key: K, value: V) {
        self.prefetch_buffer.remove(&key);

        if !self.map.contains_key(&key) && self.map.len() == self.capacity {
            self.evict_random();
        }
        self.map.insert(key, value);
    }

    /// Remove a key and return its value if it exists in the cache or prefetch buffer.
    fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(value) = self.prefetch_buffer.remove(key) {
            return Some(value);
        }
        self.map.remove(key)
    }

    /// Return the current number of entries in the main cache.
    fn len(&self) -> usize {
        self.map.len()
    }

    /// Clear all entries from the main cache and prefetch buffer.
    fn clear(&mut self) {
        self.map.clear();
        self.prefetch_buffer.clear();
    }

    /// Return the configured capacity of the cache.
    fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Implement Drop trait for safe cleanup by clearing the cache.
impl<K, V> Drop for RandomCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

/// Implement Send if the key and value types also implement Send.
unsafe impl<K, V> Send for RandomCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}

/// Implement Sync if the key and value types also implement Sync.
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
        cache.insert(3, "three"); // triggers random eviction
        assert_eq!(cache.len(), 2);
        let one_present = cache.get(&1).is_some();
        let two_present = cache.get(&2).is_some();
        let three_present = cache.get(&3).is_some();
        // At least one of the original keys remains
        assert!(one_present || two_present);
        // New key must be present
        assert!(three_present);
    }

    #[test]
    fn test_random_update_existing() {
        let mut cache = RandomCache::new(2);
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(1, "ONE"); // update key 1
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

