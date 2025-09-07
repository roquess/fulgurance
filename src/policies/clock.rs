use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::{PrefetchType, NoPrefetch};
use super::{BenchmarkablePolicy, PolicyType};

/// Clock replacement cache implementation with prefetch strategies
///
/// The Clock algorithm approximates LRU (Least Recently Used) with O(1)
/// operations. Each entry has a reference bit that acts as a "second chance"
/// before eviction. This implementation also integrates pluggable prefetch
/// strategies.
pub struct ClockCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Map from key to buffer index for O(1) lookup
    map: HashMap<K, usize>,

    /// Circular buffer of cache entries
    buffer: Vec<Option<ClockEntry<K, V>>>,

    /// Current position of the clock hand (eviction pointer)
    hand: usize,

    /// Number of active items
    len: usize,

    /// Maximum cache capacity
    capacity: usize,

    /// Prefetch strategy implementation (pluggable)
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,

    /// Prefetched but not yet used items
    prefetch_buffer: HashMap<K, V>,

    /// Limit for prefetch buffer size
    prefetch_buffer_size: usize,

    /// Prefetch statistics
    prefetch_stats: super::lru::PrefetchStats,

    /// PhantomData to bind generic types
    _marker: PhantomData<(K, V)>,
}

/// Entry stored in the clock buffer
#[derive(Clone)]
struct ClockEntry<K, V> {
    key: K,
    value: V,
    /// Reference bit - gives "second chance" before eviction
    reference_bit: bool,
}

impl<K, V> ClockEntry<K, V> {
    fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            reference_bit: true, // set reference bit on insertion
        }
    }
}

impl<K, V> ClockCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new Clock cache using the default "no prefetch" strategy
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Creates a Clock cache with a custom prefetch strategy
    pub fn with_custom_prefetch(
        capacity: usize,
        prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    ) -> Self {
        assert!(capacity > 0, "Clock cache capacity must be greater than 0");

        Self {
            map: HashMap::new(),
            buffer: vec![None; capacity],
            hand: 0,
            len: 0,
            capacity,
            prefetch_strategy,
            prefetch_buffer: HashMap::new(),
            prefetch_buffer_size: (capacity / 4).max(1),
            prefetch_stats: super::lru::PrefetchStats::default(),
            _marker: PhantomData,
        }
    }

    pub fn prefetch_stats(&self) -> &super::lru::PrefetchStats {
        &self.prefetch_stats
    }

    pub fn reset_prefetch_stats(&mut self) {
        self.prefetch_stats = super::lru::PrefetchStats::default();
        self.prefetch_strategy.reset();
    }

    /// Update prefetch strategy with new access patterns
    fn perform_prefetch(&mut self, accessed_key: &K) {
        self.prefetch_strategy.update_access_pattern(accessed_key);
        let predictions = self.prefetch_strategy.predict_next(accessed_key);

        for predicted_key in predictions {
            self.prefetch_stats.predictions_made += 1;
            if !self.map.contains_key(&predicted_key)
                && !self.prefetch_buffer.contains_key(&predicted_key)
            {
                // Prediction registered. Actual load skipped in this demo.
            }
        }
        self.trim_prefetch_buffer();
    }

    /// Ensure the prefetch buffer does not exceed the configured size
    fn trim_prefetch_buffer(&mut self) {
        while self.prefetch_buffer.len() > self.prefetch_buffer_size {
            if let Some(key) = self.prefetch_buffer.keys().next().cloned() {
                self.prefetch_buffer.remove(&key);
            } else {
                break;
            }
        }
    }

    /// Advance the clock hand until a victim slot is found
    fn advance_clock_hand(&mut self) -> usize {
        loop {
            let current_pos = self.hand;
            self.hand = (self.hand + 1) % self.capacity;

            if let Some(ref mut entry) = self.buffer[current_pos] {
                if entry.reference_bit {
                    // First chance - clear the bit
                    entry.reference_bit = false;
                } else {
                    // Victim found
                    return current_pos;
                }
            } else {
                // Empty slot found
                return current_pos;
            }
        }
    }

    /// Find index for a new entry: either free slot or evicted victim
    fn find_victim_slot(&mut self) -> usize {
        if self.len < self.capacity {
            for (i, entry) in self.buffer.iter().enumerate() {
                if entry.is_none() {
                    return i;
                }
            }
        }
        self.advance_clock_hand()
    }
}

impl<K, V> CachePolicy<K, V> for ClockCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn get(&mut self, key: &K) -> Option<&V> {
        // Check prefetch buffer first
        if let Some(_) = self.prefetch_buffer.get(key) {
            if let Some(value) = self.prefetch_buffer.remove(key) {
                self.prefetch_stats.cache_hits_from_prefetch += 1;
                self.insert(key.clone(), value);
                return self.get(key);
            }
        }

        if let Some(&index) = self.map.get(key) {
            // Avoid borrow checker conflict by ending mutable borrow
            let value_ptr: *const V;
            {
                if let Some(ref mut entry) = self.buffer[index] {
                    entry.reference_bit = true;
                    value_ptr = &entry.value;
                } else {
                    return None;
                }
            }
            // Safe to borrow `self` mutably again
            self.perform_prefetch(key);

            unsafe {
                return Some(&*value_ptr);
            }
        }
        None
    }

    fn insert(&mut self, key: K, value: V) {
        // Invalidate prefetch
        self.prefetch_buffer.remove(&key);

        if let Some(&index) = self.map.get(&key) {
            if let Some(ref mut entry) = self.buffer[index] {
                entry.value = value;
                entry.reference_bit = true;
                return;
            }
        }

        let victim_index = self.find_victim_slot();
        if let Some(ref old_entry) = self.buffer[victim_index] {
            self.map.remove(&old_entry.key);
        } else {
            self.len += 1;
        }

        let new_entry = ClockEntry::new(key.clone(), value);
        self.buffer[victim_index] = Some(new_entry);
        self.map.insert(key, victim_index);
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(value) = self.prefetch_buffer.remove(key) {
            return Some(value);
        }
        if let Some(index) = self.map.remove(key) {
            if let Some(entry) = self.buffer[index].take() {
                self.len -= 1;
                return Some(entry.value);
            }
        }
        None
    }

    fn len(&self) -> usize {
        self.len
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn clear(&mut self) {
        self.map.clear();
        for slot in &mut self.buffer {
            *slot = None;
        }
        self.hand = 0;
        self.len = 0;
        self.prefetch_buffer.clear();
    }
}

impl<K, V> BenchmarkablePolicy<K, V> for ClockCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn policy_type(&self) -> PolicyType {
        PolicyType::Clock
    }

    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}_prefetch", self.policy_type().name(), self.capacity())
    }

    fn reset_for_benchmark(&mut self) {
        self.clear();
        self.reset_prefetch_stats();
    }
}

/// Specialized constructors for concrete key types
impl ClockCache<i32, String> {
    pub fn with_prefetch_i32(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "Clock cache capacity must be greater than 0");
        let strat = crate::prefetch::create_prefetch_strategy_i32(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}
impl ClockCache<i64, String> {
    pub fn with_prefetch_i64(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "Clock cache capacity must be greater than 0");
        let strat = crate::prefetch::create_prefetch_strategy_i64(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}
impl ClockCache<usize, String> {
    pub fn with_prefetch_usize(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "Clock cache capacity must be greater than 0");
        let strat = crate::prefetch::create_prefetch_strategy_usize(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}

/// Ensure thread-safety for parallel benchmarks
unsafe impl<K, V> Send for ClockCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}
unsafe impl<K, V> Sync for ClockCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{
}

