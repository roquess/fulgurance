use std::hash::Hash;

// Exported modules of the crate
pub mod policies;
pub mod prefetch;

/// Core trait defining cache policy behavior
///
/// Types implementing this trait manage cache storage, eviction,
/// and retrieval based on a chosen policy (e.g. LRU).
pub trait CachePolicy<K, V> {
    /// Retrieve a value by key, possibly updating internal state (e.g. usage order)
    fn get(&mut self, key: &K) -> Option<&V>;

    /// Insert or update a key-value pair; may evict items if at capacity
    fn insert(&mut self, key: K, value: V);

    /// Remove a key-value pair from cache, returning the value if present
    fn remove(&mut self, key: &K) -> Option<V>;

    /// Return current number of entries in the cache
    fn len(&self) -> usize;

    /// Check if cache is empty (length is zero)
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove all entries from the cache
    fn clear(&mut self);

    /// Return the maximum allowed capacity of the cache
    fn capacity(&self) -> usize;
}

/// Trait for prefetch strategies predicting future cache accesses.
///
/// Implementations use historical or pattern data to predict keys
/// that will likely be requested soon, improving cache hit rates.
pub trait PrefetchStrategy<K> {
    /// Predict next keys likely to be accessed following the current key
    fn predict_next(&mut self, accessed_key: &K) -> Vec<K>;

    /// Update internal model/state with a new accessed key for better predictions
    fn update_access_pattern(&mut self, key: &K);

    /// Reset internal state, e.g. clearing history or counters
    fn reset(&mut self);
}

/// Struct holding statistics about cache usage and performance
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub prefetch_hits: u64,
    pub total_accesses: u64,
}

impl CacheStats {
    /// Calculate hit ratio (ratio of cache hits to total accesses)
    pub fn hit_ratio(&self) -> f64 {
        if self.total_accesses == 0 {
            0.0
        } else {
            self.hits as f64 / self.total_accesses as f64
        }
    }

    /// Calculate prefetch efficiency (ratio of useful prefetch hits)
    pub fn prefetch_efficiency(&self) -> f64 {
        if self.prefetch_hits == 0 {
            0.0
        } else {
            // This approximation assumes total prefetches = prefetch_hits + misses
            self.prefetch_hits as f64 / (self.prefetch_hits + self.misses) as f64
        }
    }

    /// Reset all tracked statistics to zero
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Composite cache struct combining an eviction policy and prefetch strategy.
///
/// Supports predictive loading and cache eviction coordination, tracking stats.
pub struct FulgranceCache<K, V, C, P>
where
    K: Clone + Hash + Eq,
    V: Clone,
    C: CachePolicy<K, V>,
    P: PrefetchStrategy<K>,
{
    cache: C,
    prefetch_strategy: P,
    prefetch_fn: Option<Box<dyn Fn(&K) -> Option<V>>>, // Custom data loader function
    stats: CacheStats,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V, C, P> FulgranceCache<K, V, C, P>
where
    K: Clone + Hash + Eq,
    V: Clone,
    C: CachePolicy<K, V>,
    P: PrefetchStrategy<K>,
{
    /// Create a new FulguranceCache combining cache and prefetch strategy
    pub fn new(cache: C, prefetch_strategy: P) -> Self {
        Self {
            cache,
            prefetch_strategy,
            prefetch_fn: None,
            stats: CacheStats::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Configure a custom prefetch function to load values on demand
    ///
    /// Function should return `Some(value)` if key can be loaded, `None` otherwise.
    pub fn with_prefetch_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&K) -> Option<V> + 'static,
    {
        self.prefetch_fn = Some(Box::new(f));
        self
    }

    /// Retrieve a value from the cache, triggering prefetching as needed
    pub fn get(&mut self, key: &K) -> Option<V> {
        self.stats.total_accesses += 1;
        // Update access pattern for prediction
        self.prefetch_strategy.update_access_pattern(key);
        // Attempt to get from cache first
        if let Some(value) = self.cache.get(key) {
            self.stats.hits += 1;
            let result = value.clone();
            // Trigger predictive prefetching of related keys
            self.prefetch_predicted_keys(key);
            return Some(result);
        }
        self.stats.misses += 1;
        // Attempt loading via prefetch function if configured
        if let Some(ref prefetch_fn) = self.prefetch_fn {
            if let Some(value) = prefetch_fn(key) {
                self.cache.insert(key.clone(), value.clone());
                self.prefetch_predicted_keys(key);
                return Some(value);
            }
        }
        None
    }

    /// Insert or update a key-value pair directly in the cache
    pub fn insert(&mut self, key: K, value: V) {
        self.cache.insert(key, value);
    }

    /// Remove a key-value pair from the cache, returning its value if present
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.cache.remove(key)
    }

    /// Access current cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Reset cache and prefetch strategy statistics and state
    pub fn reset_stats(&mut self) {
        self.stats.reset();
        self.prefetch_strategy.reset();
    }

    /// Get current number of items in the cache
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get cache capacity
    pub fn capacity(&self) -> usize {
        self.cache.capacity()
    }

    /// Clear all data in the cache and reset statistics
    pub fn clear(&mut self) {
        self.cache.clear();
        self.reset_stats();
    }

    /// Internal helper to prefetch keys predicted by prefetch strategy
    fn prefetch_predicted_keys(&mut self, accessed_key: &K) {
        if let Some(ref prefetch_fn) = self.prefetch_fn {
            let predicted_keys = self.prefetch_strategy.predict_next(accessed_key);
            for key in predicted_keys {
                // Only fetch if key not already cached
                if self.cache.get(&key).is_none() {
                    if let Some(value) = prefetch_fn(&key) {
                        self.cache.insert(key, value);
                        self.stats.prefetch_hits += 1;
                    }
                }
            }
        }
    }
}

// Convenient re-exports for common types and modules
pub mod prelude {
    pub use super::{CachePolicy, PrefetchStrategy, FulgranceCache, CacheStats};
    pub use super::policies::{LruCache, MruCache, PolicyType};
    pub use super::prefetch::{SequentialPrefetch, PrefetchType};
}
