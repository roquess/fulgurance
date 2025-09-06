use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::{PrefetchType, NoPrefetch};
use super::{BenchmarkablePolicy, PolicyType};

/// A First-In-First-Out (FIFO) cache implementation with optional prefetch strategies.
///
/// # Overview
/// - This cache evicts the oldest inserted item when its capacity is exceeded.
/// - Provides O(1) average complexity for `get` and `insert`.
/// - Provides O(n) complexity for `remove` (due to searching in `VecDeque`).
///
/// # Prefetching
/// The cache integrates with **prefetch strategies** to predict and preload
/// likely future accesses. This improves performance in workloads with predictable patterns.
pub struct FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Primary key-value store
    map: HashMap<K, V>,

    /// Queue to keep track of insertion order (oldest -> newest)
    order: VecDeque<K>,

    /// Maximum number of items that can be stored
    capacity: usize,

    /// Strategy used for making prefetch predictions
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,

    /// Buffer that stores prefetched (but not yet used) values
    prefetch_buffer: HashMap<K, V>,

    /// Maximum size allowed for the prefetch buffer
    prefetch_buffer_size: usize,

    /// Statistics that track prefetch efficiency
    prefetch_stats: PrefetchStats,
}

/// Statistics for evaluating the effectiveness of prefetching
#[derive(Debug, Clone, Default)]
pub struct PrefetchStats {
    /// Number of predictions that were generated
    pub predictions_made: u64,

    /// Times a prefetched key was later accessed (successful prediction)
    pub prefetch_hits: u64,

    /// Times a prefetched key was not accessed (wasted prediction)
    pub prefetch_misses: u64,

    /// Number of accesses directly satisfied from prefetched values
    pub cache_hits_from_prefetch: u64,
}

impl PrefetchStats {
    /// Calculate the hit rate: (prefetch hits / total predictions) * 100
    pub fn hit_rate(&self) -> f64 {
        if self.predictions_made == 0 {
            0.0
        } else {
            (self.prefetch_hits as f64 / self.predictions_made as f64) * 100.0
        }
    }

    /// Calculate how often prefetched elements are actually used:
    /// (cache hits from prefetch / total prefetch hits) * 100
    pub fn effectiveness(&self) -> f64 {
        if self.prefetch_hits == 0 {
            0.0
        } else {
            (self.cache_hits_from_prefetch as f64 / self.prefetch_hits as f64) * 100.0
        }
    }
}

impl<K, V> FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new FIFO cache with capacity `capacity` using `NoPrefetch` as strategy.
    ///
    /// # Panics
    /// Panics if capacity is set to `0`.
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Create a FIFO cache with a custom prefetch strategy.
    pub fn with_custom_prefetch(
        capacity: usize,
        prefetch_strategy: Box<dyn PrefetchStrategy<K>>
    ) -> Self {
        assert!(capacity > 0, "FIFO cache capacity must be greater than 0");
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
            prefetch_strategy,
            prefetch_buffer: HashMap::new(),
            prefetch_buffer_size: (capacity / 4).max(1),
            prefetch_stats: PrefetchStats::default(),
        }
    }

    /// Create a cache with a default capacity of **100 entries**
    pub fn with_default_capacity() -> Self {
        Self::new(100)
    }

    /// Return the current prefetch statistics
    pub fn prefetch_stats(&self) -> &PrefetchStats {
        &self.prefetch_stats
    }

    /// Reset prefetch statistics and strategy state
    pub fn reset_prefetch_stats(&mut self) {
        self.prefetch_stats = PrefetchStats::default();
        self.prefetch_strategy.reset();
    }

    /// Set a new maximum size for the prefetch buffer
    pub fn set_prefetch_buffer_size(&mut self, size: usize) {
        self.prefetch_buffer_size = size.max(1);
        self.trim_prefetch_buffer();
    }

    /// Ensure the prefetch buffer does not exceed the specified maximum size
    fn trim_prefetch_buffer(&mut self) {
        while self.prefetch_buffer.len() > self.prefetch_buffer_size {
            if let Some(key) = self.prefetch_buffer.keys().next().cloned() {
                self.prefetch_buffer.remove(&key);
            } else {
                break;
            }
        }
    }

    /// Perform prefetching based on the current access and update the buffer
    fn perform_prefetch(&mut self, accessed_key: &K) {
        self.prefetch_strategy.update_access_pattern(accessed_key);

        // Ask the strategy for predicted next keys
        let predictions = self.prefetch_strategy.predict_next(accessed_key);

        for predicted_key in predictions {
            self.prefetch_stats.predictions_made += 1;

            // Only store prediction if it's not already in cache or buffer
            if !self.map.contains_key(&predicted_key)
                && !self.prefetch_buffer.contains_key(&predicted_key) {
                // In a real cache, value would be loaded from storage/datasource
                // Here, we simulate predictions without actually fetching values
            }
        }

        self.trim_prefetch_buffer();
    }

    /// Evict the **oldest** key (front of the queue)
    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self.order.pop_front() {
            self.map.remove(&oldest_key);
        }
    }

    /// Returns true if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// Specialized constructors for supported key types
impl FifoCache<i32, String> {
    pub fn with_prefetch_i32(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "FIFO cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_i32(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl FifoCache<i64, String> {
    pub fn with_prefetch_i64(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "FIFO cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_i64(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl FifoCache<usize, String> {
    pub fn with_prefetch_usize(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "FIFO cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_usize(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl<K, V> CachePolicy<K, V> for FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Retrieve a value reference by key.
    ///
    /// - If the item exists in the prefetch buffer, it is moved into the main cache first.
    /// - If found in the main cache, prefetching is triggered.
    fn get(&mut self, key: &K) -> Option<&V> {
        // Check prefetch buffer first
        if let Some(_) = self.prefetch_buffer.get(key) {
            if let Some(value) = self.prefetch_buffer.remove(key) {
                self.prefetch_stats.cache_hits_from_prefetch += 1;
                self.insert(key.clone(), value);
                return self.get(key);
            }
        }

        // Safe version: check presence, then evaluate again after prefetch
        if self.map.contains_key(key) {
            self.perform_prefetch(key);
            self.map.get(key)
        } else {
            None
        }
    }

    /// Insert a new key-value pair into the cache.
    ///
    /// - If the key already exists, update its value without changing order.
    /// - Evicts the oldest item if capacity is exceeded.
    fn insert(&mut self, key: K, value: V) {
        self.prefetch_buffer.remove(&key);

        if !self.map.contains_key(&key) {
            if self.map.len() == self.capacity {
                self.evict_oldest();
            }
            self.order.push_back(key.clone());
        }
        self.map.insert(key, value);
    }

    /// Remove a key and return its value if present
    fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(value) = self.prefetch_buffer.remove(key) {
            return Some(value);
        }

        if let Some(value) = self.map.remove(key) {
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                self.order.remove(pos);
            }
            Some(value)
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
        self.prefetch_buffer.clear();
    }

    fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<K, V> BenchmarkablePolicy<K, V> for FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn policy_type(&self) -> PolicyType {
        PolicyType::Fifo
    }

    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}_prefetch", self.policy_type().name(), self.capacity())
    }

    fn reset_for_benchmark(&mut self) {
        self.clear();
        self.reset_prefetch_stats();
    }
}

impl<K, V> Drop for FifoCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

// Allow the cache to be safely shared across threads if K & V are Send/Sync
unsafe impl<K, V> Send for FifoCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{}
unsafe impl<K, V> Sync for FifoCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{}
