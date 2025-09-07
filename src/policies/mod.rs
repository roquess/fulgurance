//! Cache policy implementations containing eviction strategies implementing CachePolicy.

use std::hash::Hash;
use crate::CachePolicy;

pub mod lru;
pub mod mru;
pub mod fifo;
pub mod lfu;
pub mod random;
pub mod arc;
pub mod clock;
pub mod two_q;
pub mod slru;
pub mod car;

pub use lru::LruCache;
pub use mru::MruCache;
pub use fifo::FifoCache;
pub use lfu::LfuCache;
pub use random::RandomCache;
pub use arc::ArcCache;
pub use clock::ClockCache;
pub use two_q::TwoQCache;
pub use slru::SlruCache;
pub use car::CarCache;

/// Supported cache policies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicyType {
    Lru,
    Mru,
    Fifo,
    Lfu,
    Random,
    Arc,
    Clock,
    TwoQ,
    Slru,
    Car,
}

impl PolicyType {
    pub fn name(&self) -> &'static str {
        match self {
            PolicyType::Lru => "LRU",
            PolicyType::Mru => "MRU",
            PolicyType::Fifo => "FIFO",
            PolicyType::Lfu => "LFU",
            PolicyType::Random => "Random",
            PolicyType::Arc => "ARC",
            PolicyType::Clock => "Clock",
            PolicyType::TwoQ => "2Q",
            PolicyType::Slru => "SLRU",
            PolicyType::Car => "CAR",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PolicyType::Lru => "Evicts the least recently used item",
            PolicyType::Mru => "Evicts the most recently used item",
            PolicyType::Fifo => "Evicts items in first-in-first-out order",
            PolicyType::Lfu => "Evicts the least frequently used item",
            PolicyType::Random => "Evicts a random item",
            PolicyType::Arc => "Adaptively balances between recency and frequency",
            PolicyType::Clock => "Uses circular buffer with reference bits for approximated LRU",
            PolicyType::TwoQ => "Two-queue combining FIFO & LRU for scan resistance",
            PolicyType::Slru => "Segmented LRU with probationary and protected segments",
            PolicyType::Car => "Clock with adaptive replacement like ARC",
        }
    }

    pub fn all() -> &'static [PolicyType] {
        &[
            PolicyType::Lru,
            PolicyType::Mru,
            PolicyType::Fifo,
            PolicyType::Lfu,
            PolicyType::Random,
            PolicyType::Arc,
            PolicyType::Clock,
            PolicyType::TwoQ,
            PolicyType::Slru,
            PolicyType::Car,
        ]
    }

    pub fn advanced() -> &'static [PolicyType] {
        &[
            PolicyType::Arc,
            PolicyType::TwoQ,
            PolicyType::Slru,
            PolicyType::Car,
        ]
    }

    pub fn simple() -> &'static [PolicyType] {
        &[
            PolicyType::Lru,
            PolicyType::Fifo,
            PolicyType::Clock,
            PolicyType::Random,
        ]
    }

    pub fn is_adaptive(&self) -> bool {
        matches!(self, PolicyType::Arc | PolicyType::Car)
    }

    pub fn is_scan_resistant(&self) -> bool {
        matches!(self, PolicyType::Arc | PolicyType::TwoQ | PolicyType::Slru | PolicyType::Car)
    }
}

/// Factory returning boxed BenchmarkablePolicy trait object with explicit generic parameters
pub fn create_cache_policy<K, V>(
    policy_type: PolicyType,
    capacity: usize,
) -> Box<dyn BenchmarkablePolicy<K, V>>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    match policy_type {
        PolicyType::Lru => Box::new(LruCache::<K, V>::new(capacity)),
        PolicyType::Mru => Box::new(MruCache::<K, V>::new(capacity)),
        PolicyType::Fifo => Box::new(FifoCache::<K, V>::new(capacity)),
        PolicyType::Lfu => Box::new(LfuCache::<K, V>::new(capacity)),
        PolicyType::Random => Box::new(RandomCache::<K, V>::new(capacity)),
        PolicyType::Arc => Box::new(ArcCache::<K, V>::new(capacity)),
        PolicyType::Clock => Box::new(ClockCache::<K, V>::new(capacity)),
        PolicyType::TwoQ => Box::new(TwoQCache::<K, V>::new(capacity)),
        PolicyType::Slru => Box::new(SlruCache::<K, V>::new(capacity)),
        PolicyType::Car => Box::new(CarCache::<K, V>::new(capacity)),
    }
}

/// Trait extension of CachePolicy for benchmarking support
pub trait BenchmarkablePolicy<K, V>: CachePolicy<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn policy_type(&self) -> PolicyType;

    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}", self.policy_type().name(), self.capacity())
    }

    fn reset_for_benchmark(&mut self) {
        self.clear();
    }

    fn benchmark_operations(&mut self, operations: &[(K, Option<V>)]) {
        for (key, maybe_value) in operations {
            if let Some(value) = maybe_value {
                self.insert(key.clone(), value.clone());
            } else {
                self.get(key);
            }
        }
    }

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
            PolicyType::Fifo => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)",
                memory_overhead: "Low",
                cache_friendly: true,
                temporal_locality: false,
                spatial_locality: false,
            },
            PolicyType::Lfu => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)",
                memory_overhead: "High",
                cache_friendly: false,
                temporal_locality: false,
                spatial_locality: false,
            },
            PolicyType::Random => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)",
                memory_overhead: "Low",
                cache_friendly: false,
                temporal_locality: false,
                spatial_locality: false,
            },
            PolicyType::Arc => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)",
                memory_overhead: "Very High",
                cache_friendly: true,
                temporal_locality: true,
                spatial_locality: false,
            },
            PolicyType::Clock => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)*",
                memory_overhead: "Medium",
                cache_friendly: true,
                temporal_locality: true,
                spatial_locality: false,
            },
            PolicyType::TwoQ => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)",
                memory_overhead: "High",
                cache_friendly: true,
                temporal_locality: true,
                spatial_locality: false,
            },
            PolicyType::Slru => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)",
                memory_overhead: "High",
                cache_friendly: true,
                temporal_locality: true,
                spatial_locality: false,
            },
            PolicyType::Car => PolicyCharacteristics {
                avg_get_complexity: "O(1)",
                avg_insert_complexity: "O(1)*",
                memory_overhead: "High",
                cache_friendly: true,
                temporal_locality: true,
                spatial_locality: false,
            },
        }
    }
}

/// Describes performance traits of a cache policy
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

