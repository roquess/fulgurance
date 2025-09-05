use std::collections::{HashMap, BTreeMap};
use std::hash::Hash;
use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::{PrefetchType, NoPrefetch};
use super::{BenchmarkablePolicy, PolicyType};

/// A Least Frequently Used (LFU) cache implementation with integrated prefetch strategies
///
/// This cache evicts the item with the lowest access frequency.
/// When multiple keys have the same frequency, the oldest inserted among them is evicted.
/// The cache integrates with prefetch strategies to predict and preload
/// likely future accesses, improving performance for predictable access patterns.
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
    /// Prefetch strategy for predicting future accesses
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    /// Prefetch buffer to store preloaded values
    prefetch_buffer: HashMap<K, V>,
    /// Maximum size of prefetch buffer
    prefetch_buffer_size: usize,
    /// Statistics for prefetch effectiveness
    prefetch_stats: PrefetchStats,
}

/// Statistics tracking prefetch effectiveness
#[derive(Debug, Clone, Default)]
pub struct PrefetchStats {
    /// Number of prefetch predictions made
    pub predictions_made: u64,
    /// Number of prefetch hits (predicted key was actually accessed)
    pub prefetch_hits: u64,
    /// Number of prefetch misses (predicted key was not accessed)
    pub prefetch_misses: u64,
    /// Number of cache hits from prefetched data
    pub cache_hits_from_prefetch: u64,
}

impl PrefetchStats {
    /// Calculate prefetch hit rate as a percentage
    pub fn hit_rate(&self) -> f64 {
        if self.predictions_made == 0 {
            0.0
        } else {
            (self.prefetch_hits as f64 / self.predictions_made as f64) * 100.0
        }
    }

    /// Calculate prefetch effectiveness (cache hits from prefetch / total prefetch hits)
    pub fn effectiveness(&self) -> f64 {
        if self.prefetch_hits == 0 {
            0.0
        } else {
            (self.cache_hits_from_prefetch as f64 / self.prefetch_hits as f64) * 100.0
        }
    }
}

impl<K, V> LfuCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new LFU cache with no prefetch (baseline)
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of items the cache can hold
    ///
    /// # Panics
    /// Panics if capacity is 0
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Creates a new LFU cache with custom prefetch strategy
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of items the cache can hold
    /// * `prefetch_strategy` - Custom prefetch strategy implementation
    ///
    /// # Panics
    /// Panics if capacity is 0
    pub fn with_custom_prefetch(
        capacity: usize,
        prefetch_strategy: Box<dyn PrefetchStrategy<K>>
    ) -> Self {
        assert!(capacity > 0, "LFU cache capacity must be greater than 0");

        Self {
            map: HashMap::new(),
            freq_list: BTreeMap::new(),
            capacity,
            min_freq: 0,
            prefetch_strategy,
            prefetch_buffer: HashMap::new(),
            prefetch_buffer_size: (capacity / 4).max(1),
            prefetch_stats: PrefetchStats::default(),
        }
    }

    /// Creates an LFU cache with default capacity 100
    pub fn with_default_capacity() -> Self {
        Self::new(100)
    }

    /// Returns current prefetch statistics
    pub fn prefetch_stats(&self) -> &PrefetchStats {
        &self.prefetch_stats
    }

    /// Resets prefetch statistics
    pub fn reset_prefetch_stats(&mut self) {
        self.prefetch_stats = PrefetchStats::default();
        self.prefetch_strategy.reset();
    }

    /// Sets the prefetch buffer size
    pub fn set_prefetch_buffer_size(&mut self, size: usize) {
        self.prefetch_buffer_size = size.max(1);
        self.trim_prefetch_buffer();
    }

    /// Trims the prefetch buffer to the specified size
    fn trim_prefetch_buffer(&mut self) {
        while self.prefetch_buffer.len() > self.prefetch_buffer_size {
            if let Some(key) = self.prefetch_buffer.keys().next().cloned() {
                self.prefetch_buffer.remove(&key);
            } else {
                break;
            }
        }
    }

    /// Performs prefetch predictions and populates the prefetch buffer
    fn perform_prefetch(&mut self, accessed_key: &K) {
        // Update prefetch strategy with the accessed key
        self.prefetch_strategy.update_access_pattern(accessed_key);

        // Get predictions from the strategy
        let predictions = self.prefetch_strategy.predict_next(accessed_key);

        for predicted_key in predictions {
            self.prefetch_stats.predictions_made += 1;

            // Only prefetch if the key is not already in main cache or prefetch buffer
            if !self.map.contains_key(&predicted_key) &&
               !self.prefetch_buffer.contains_key(&predicted_key) {

                // Here you would typically load the value from your data source
                // For now, we'll simulate with a placeholder
                // In a real implementation, this would be:
                // if let Some(value) = self.load_from_source(&predicted_key) {
                //     self.prefetch_buffer.insert(predicted_key, value);
                // }

                // For demonstration, we'll skip actual prefetch loading
                // but track the prediction
            }
        }

        // Trim prefetch buffer if it exceeds size limit
        self.trim_prefetch_buffer();
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

// Specialized constructors for types that support our prefetch strategies
impl LfuCache<i32, String> {
    /// Creates a new i32 LFU cache with specified prefetch strategy
    pub fn with_prefetch_i32(capacity: usize, prefetch_type: PrefetchType) -> Self {
        use crate::prefetch::{SequentialPrefetch, MarkovPrefetch};

        assert!(capacity > 0, "LFU cache capacity must be greater than 0");

        let prefetch_strategy: Box<dyn PrefetchStrategy<i32>> = match prefetch_type {
            PrefetchType::Sequential => Box::new(SequentialPrefetch::<i32>::new()),
            PrefetchType::Markov => Box::new(MarkovPrefetch::<i32>::new()),
            PrefetchType::None => Box::new(NoPrefetch),
        };

        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl LfuCache<i64, String> {
    /// Creates a new i64 LFU cache with specified prefetch strategy
    pub fn with_prefetch_i64(capacity: usize, prefetch_type: PrefetchType) -> Self {
        use crate::prefetch::{SequentialPrefetch, MarkovPrefetch};

        assert!(capacity > 0, "LFU cache capacity must be greater than 0");

        let prefetch_strategy: Box<dyn PrefetchStrategy<i64>> = match prefetch_type {
            PrefetchType::Sequential => Box::new(SequentialPrefetch::<i64>::new()),
            PrefetchType::Markov => Box::new(MarkovPrefetch::<i64>::new()),
            PrefetchType::None => Box::new(NoPrefetch),
        };

        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl LfuCache<usize, String> {
    /// Creates a new usize LFU cache with specified prefetch strategy
    pub fn with_prefetch_usize(capacity: usize, prefetch_type: PrefetchType) -> Self {
        use crate::prefetch::{SequentialPrefetch, MarkovPrefetch};

        assert!(capacity > 0, "LFU cache capacity must be greater than 0");

        let prefetch_strategy: Box<dyn PrefetchStrategy<usize>> = match prefetch_type {
            PrefetchType::Sequential => Box::new(SequentialPrefetch::<usize>::new()),
            PrefetchType::Markov => Box::new(MarkovPrefetch::<usize>::new()),
            PrefetchType::None => Box::new(NoPrefetch),
        };

        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl<K, V> CachePolicy<K, V> for LfuCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Gets value by key and increases its frequency
    fn get(&mut self, key: &K) -> Option<&V> {
        // Check if it's in the prefetch buffer first
        if let Some(_) = self.prefetch_buffer.get(key) {
            // Move from prefetch buffer to main cache
            if let Some(value) = self.prefetch_buffer.remove(key) {
                self.prefetch_stats.cache_hits_from_prefetch += 1;
                self.insert(key.clone(), value);
                return self.get(key); // Recursive call to get from main cache
            }
        }

        if self.map.contains_key(key) {
            self.increase_freq(key);
            // Perform prefetch predictions
            self.perform_prefetch(key);
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

        // Remove from prefetch buffer if it exists there
        self.prefetch_buffer.remove(&key);

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
        // Check prefetch buffer first
        if let Some(value) = self.prefetch_buffer.remove(key) {
            return Some(value);
        }

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
        self.prefetch_buffer.clear();
    }

    /// Returns maximal capacity allowed
    fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<K, V> BenchmarkablePolicy<K, V> for LfuCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Returns the policy type for this cache
    fn policy_type(&self) -> PolicyType {
        PolicyType::Lfu
    }

    /// Returns a standardized string identifier for benchmarking reports
    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}_prefetch", self.policy_type().name(), self.capacity())
    }

    /// Resets the internal cache state for consistent benchmarking
    fn reset_for_benchmark(&mut self) {
        self.clear();
        self.reset_prefetch_stats();
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

