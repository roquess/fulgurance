//! Cache policy implementations
//!
//! This module contains various cache eviction policies that implement
//! the `CachePolicy` trait. Each policy manages cache entries according
//! to different strategies for optimal performance in various scenarios.

use std::hash::Hash;
use crate::CachePolicy;

pub mod lru;
pub mod mru;
pub mod fifo;
pub mod lfu;
pub mod random;

// Re-export all available cache policies to make them accessible externally
pub use lru::LruCache;
pub use mru::MruCache;
pub use fifo::FifoCache;
pub use lfu::LfuCache;
pub use random::RandomCache;

/// Enum representing all supported cache policy types
///
/// Allows dynamic runtime selection and switching of cache policies for benchmarking and flexibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicyType {
    /// Least Recently Used - evicts the least recently accessed item
    Lru,
    /// Most Recently Used - evicts the most recently accessed item
    Mru,
    /// First In, First Out - evicts items in insertion order
    Fifo,
    /// Least Frequently Used - evicts items with lowest usage count
    Lfu,
    /// Random Eviction - evicts a random item
    Random,
}

impl PolicyType {
    /// Returns the user-friendly name of the cache policy
    pub fn name(&self) -> &'static str {
        match self {
            PolicyType::Lru => "LRU",
            PolicyType::Mru => "MRU",
            PolicyType::Fifo => "FIFO",
            PolicyType::Lfu => "LFU",
            PolicyType::Random => "Random",
        }
    }

    /// Returns a short description of the eviction strategy used by the policy
    pub fn description(&self) -> &'static str {
        match self {
            PolicyType::Lru => "Evicts the least recently used item",
            PolicyType::Mru => "Evicts the most recently used item",
            PolicyType::Fifo => "Evicts items in first-in-first-out order",
            PolicyType::Lfu => "Evicts the least frequently used item",
            PolicyType::Random => "Evicts a random item",
        }
    }

    /// Returns a static list of all available policy types for iteration or display
    pub fn all() -> &'static [PolicyType] {
        &[
            PolicyType::Lru,
            PolicyType::Mru,
            PolicyType::Fifo,
            PolicyType::Lfu,
            PolicyType::Random,
        ]
    }
}

/// Factory function to create cache policies dynamically at runtime
///
/// This function enables flexible policy selection for various contexts,
/// such as benchmarking or adaptive caching algorithms.
pub fn create_cache_policy<K, V>(
    policy_type: PolicyType,
    capacity: usize,
) -> Box<dyn CachePolicy<K, V>>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    match policy_type {
        PolicyType::Lru => Box::new(LruCache::new(capacity)),
        PolicyType::Mru => Box::new(MruCache::new(capacity)),
        PolicyType::Fifo => Box::new(FifoCache::new(capacity)),
        PolicyType::Lfu => Box::new(LfuCache::new(capacity)),
        PolicyType::Random => Box::new(RandomCache::new(capacity)), // Instantiate Random cache here
    }
}

/// Trait extension for cache policies supporting benchmarking functionality
///
/// Provides access to policy metadata, batch operation support,
/// and performance characteristic reporting for benchmarking tools.
pub trait BenchmarkablePolicy<K, V>: CachePolicy<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Returns the enum identifying the policy type
    fn policy_type(&self) -> PolicyType;

    /// Returns a standardized string identifier for benchmarking reports
    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}", self.policy_type().name(), self.capacity())
    }

    /// Resets the internal cache state for consistent benchmarking
    fn reset_for_benchmark(&mut self) {
        self.clear();
    }

    /// Executes a batch of insert/get operations for benchmarking purposes
    ///
    /// `operations` is a slice of tuples where each tuple holds a key and an optional value.
    /// If `Some(value)`, performs insert; if `None`, performs get.
    fn benchmark_operations(&mut self, operations: &[(K, Option<V>)]) {
        for (key, maybe_value) in operations {
            if let Some(value) = maybe_value {
                self.insert(key.clone(), value.clone());
            } else {
                self.get(key);
            }
        }
    }

    /// Returns performance and behavior characteristics of the cache policy
    fn characteristics(&self) -> PolicyCharacteristics {
        match self.policy_type() {
            PolicyType::Lru => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)",
                memory_overhead: "High",
                cache_friendly: true,
                temporal_locality: true,
                spatial_locality: false,
            },
            PolicyType::Mru => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)",
                memory_overhead: "High",
                cache_friendly: false,
                temporal_locality: false,
                spatial_locality: false,
            },
            _ => PolicyCharacteristics::default(),
        }
    }
}

/// Struct representing performance characteristics of a cache policy
#[derive(Debug, Clone)]
pub struct PolicyCharacteristics {
    /// Average time complexity of `get` operations
    pub avg_get_complexity: &'static str,
    /// Average time complexity of `insert` operations
    pub avg_insert_complexity: &'static str,
    /// Memory overhead classification (e.g. "High", "Low")
    pub memory_overhead: &'static str,
    /// Whether the policy is cache-line friendly
    pub cache_friendly: bool,
    /// Whether the policy benefits from temporal locality
    pub temporal_locality: bool,
    /// Whether the policy benefits from spatial locality
    pub spatial_locality: bool,
}

impl Default for PolicyCharacteristics {
    fn default() -> Self {
        Self {
            avg_get_complexity: "Unknown",
            avg_insert_complexity: "Unknown",
            memory_overhead: "Unknown",
            cache_friendly: false,
            temporal_locality: false,
            spatial_locality: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_type_enum() {
        // Verify policy names are correct strings
        assert_eq!(PolicyType::Lru.name(), "LRU");
        assert_eq!(PolicyType::Mru.name(), "MRU");

        // Verify descriptions contain key phrases
        assert!(PolicyType::Lru.description().contains("least recently"));
        assert!(PolicyType::Mru.description().contains("most recently"));

        // Verify all policies exist in the policy list
        let all_policies = PolicyType::all();
        assert!(all_policies.contains(&PolicyType::Lru));
        assert!(all_policies.contains(&PolicyType::Mru));
    }

    #[test]
    fn test_factory_pattern() {
        // Create all policies and verify capacity correctness
        let lru_cache = create_cache_policy::<i32, String>(PolicyType::Lru, 100);
        let mru_cache = create_cache_policy::<i32, String>(PolicyType::Mru, 50);
        let fifo_cache = create_cache_policy::<i32, String>(PolicyType::Fifo, 30);
        let lfu_cache = create_cache_policy::<i32, String>(PolicyType::Lfu, 20);
        let random_cache = create_cache_policy::<i32, String>(PolicyType::Random, 10);
        assert_eq!(lru_cache.capacity(), 100);
        assert_eq!(mru_cache.capacity(), 50);
        assert_eq!(fifo_cache.capacity(), 30);
        assert_eq!(lfu_cache.capacity(), 20);
        assert_eq!(random_cache.capacity(), 10);
    }
}

