//! Cache policy implementations
//!
//! This module contains various cache eviction policies that implement
//! the `CachePolicy` trait. Each policy manages cache entries according
//! to different strategies for optimal performance in various scenarios.

use std::hash::Hash;
use crate::CachePolicy;

pub mod lru;
pub mod mru;
// pub mod fifo; // Will be added later
// pub mod lfu;  // Will be added later

// Re-export all policy implementations
pub use lru::LruCache;
pub use mru::MruCache;
// pub use fifo::FifoCache;  // Will be uncommented when implemented
// pub use lfu::LfuCache;    // Will be uncommented when implemented

/// Enumeration of available cache policy types
///
/// This enum allows dynamic selection of cache policies at runtime
/// and facilitates easy switching between different strategies for benchmarking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicyType {
    /// Least Recently Used - evicts the item that was accessed longest ago
    Lru,
    /// Most Recently Used - evicts the most recently accessed item
    Mru,
    /// First In, First Out - evicts items in the order they were added
    Fifo,
    /// Least Frequently Used - evicts the item with the lowest access count
    Lfu,
    /// Random - evicts a random item
    Random,
}

impl PolicyType {
    /// Returns a human-readable name for the policy
    pub fn name(&self) -> &'static str {
        match self {
            PolicyType::Lru => "LRU",
            PolicyType::Mru => "MRU",
            PolicyType::Fifo => "FIFO",
            PolicyType::Lfu => "LFU",
            PolicyType::Random => "Random",
        }
    }

    /// Returns a description of the policy's behavior
    pub fn description(&self) -> &'static str {
        match self {
            PolicyType::Lru => "Evicts the least recently used item",
            PolicyType::Mru => "Evicts the most recently used item",
            PolicyType::Fifo => "Evicts items in first-in-first-out order",
            PolicyType::Lfu => "Evicts the least frequently used item",
            PolicyType::Random => "Evicts a random item",
        }
    }

    /// Returns all available policy types
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

/// Factory function to create cache policies dynamically
///
/// This function enables runtime selection of cache policies, which is
/// particularly useful for benchmarking different strategies.
pub fn create_cache_policy<K, V>(
    policy_type: PolicyType,
    capacity: usize
) -> Box<dyn CachePolicy<K, V>>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    match policy_type {
        PolicyType::Lru => Box::new(LruCache::new(capacity)),
        PolicyType::Mru => Box::new(MruCache::new(capacity)),
        PolicyType::Fifo => {
            // TODO: Implement FIFO
            unimplemented!("FIFO cache policy not yet implemented")
        },
        PolicyType::Lfu => {
            // TODO: Implement LFU
            unimplemented!("LFU cache policy not yet implemented")
        },
        PolicyType::Random => {
            // TODO: Implement Random
            unimplemented!("Random cache policy not yet implemented")
        },
    }
}

/// Trait for cache policies that support benchmarking
///
/// This trait extends the basic `CachePolicy` with additional methods
/// needed for comprehensive benchmarking and performance analysis.
pub trait BenchmarkablePolicy<K, V>: CachePolicy<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Returns the policy type identifier
    fn policy_type(&self) -> PolicyType;

    /// Returns a name suitable for benchmark reports
    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}", self.policy_type().name(), self.capacity())
    }

    /// Resets internal state for consistent benchmarking
    fn reset_for_benchmark(&mut self) {
        self.clear();
    }

    /// Performs a batch of operations for benchmarking
    fn benchmark_operations(&mut self, operations: &[(K, Option<V>)]) {
        for (key, maybe_value) in operations {
            if let Some(value) = maybe_value {
                self.insert(key.clone(), value.clone());
            } else {
                self.get(key);
            }
        }
    }

    /// Returns performance characteristics of this policy
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

/// Performance characteristics of a cache policy
#[derive(Debug, Clone)]
pub struct PolicyCharacteristics {
    pub avg_get_complexity: &'static str,
    pub avg_insert_complexity: &'static str,
    pub memory_overhead: &'static str,
    pub cache_friendly: bool,
    pub temporal_locality: bool,
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
        // Test enum properties
        assert_eq!(PolicyType::Lru.name(), "LRU");
        assert_eq!(PolicyType::Mru.name(), "MRU");
        assert!(PolicyType::Lru.description().contains("least recently"));
        assert!(PolicyType::Mru.description().contains("most recently"));
        
        // Test all policies are listed
        let all_policies = PolicyType::all();
        assert!(all_policies.contains(&PolicyType::Lru));
        assert!(all_policies.contains(&PolicyType::Mru));
    }

    #[test]
    fn test_factory_pattern() {
        // Test that factory creates correct types
        let lru_cache = create_cache_policy::<i32, String>(PolicyType::Lru, 100);
        let mru_cache = create_cache_policy::<i32, String>(PolicyType::Mru, 50);
        
        assert_eq!(lru_cache.capacity(), 100);
        assert_eq!(mru_cache.capacity(), 50);
    }

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_unimplemented_policies() {
        // Test that unimplemented policies panic appropriately
        let _cache = create_cache_policy::<i32, String>(PolicyType::Fifo, 100);
    }
}
