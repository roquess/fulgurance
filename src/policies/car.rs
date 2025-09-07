use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::{PrefetchType, NoPrefetch};
use super::{BenchmarkablePolicy, PolicyType};

/// Clock with Adaptive Replacement (CAR) cache
///
/// CAR combines the Clock algorithm with ARC’s adaptive mechanism.
/// Like ARC, it maintains four lists (two actual cache lists + two
/// ghost lists) but uses Clock replacement instead of strict LRU.
/// This provides adaptive behavior with lower overhead.
pub struct CarCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    // T1: Recent entries (Clock-managed)
    t1: Vec<Option<CarEntry<K, V>>>,
    t1_map: HashMap<K, usize>,
    t1_hand: usize,
    t1_size: usize,

    // T2: Frequent entries (Clock-managed)
    t2: Vec<Option<CarEntry<K, V>>>,
    t2_map: HashMap<K, usize>,
    t2_hand: usize,
    t2_size: usize,

    // Ghost buffers store only keys, used for adaptation
    b1: HashMap<K, ()>, // Ghost buffer for T1 evictions
    b2: HashMap<K, ()>, // Ghost buffer for T2 evictions

    // Adaptation parameter (target size of T1)
    p: usize,

    // Capacity and current size
    capacity: usize,
    current_size: usize,

    // Integrated prefetch support
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    prefetch_buffer: HashMap<K, V>,
    prefetch_buffer_size: usize,
    prefetch_stats: super::lru::PrefetchStats,

    _marker: PhantomData<(K, V)>,
}

/// An entry managed by the CAR cache
#[derive(Clone)]
struct CarEntry<K, V> {
    key: K,
    value: V,
    reference_bit: bool, // Clock reference bit
    #[allow(dead_code)]
    list_type: ListType, // Indicates whether in T1 or T2
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ListType {
    T1,
    T2,
}

impl<K, V> CarEntry<K, V> {
    fn new(key: K, value: V, list_type: ListType) -> Self {
        Self {
            key,
            value,
            reference_bit: true,
            list_type,
        }
    }
}

impl<K, V> CarCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new CAR cache with no prefetch
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Creates a new CAR cache with given prefetch strategy
    pub fn with_custom_prefetch(
        capacity: usize,
        prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    ) -> Self {
        assert!(capacity > 0, "CAR cache capacity must be greater than 0");

        let t1_capacity = capacity;
        let t2_capacity = capacity;

        Self {
            t1: vec![None; t1_capacity],
            t1_map: HashMap::new(),
            t1_hand: 0,
            t1_size: 0,

            t2: vec![None; t2_capacity],
            t2_map: HashMap::new(),
            t2_hand: 0,
            t2_size: 0,

            b1: HashMap::new(),
            b2: HashMap::new(),

            p: 0,
            capacity,
            current_size: 0,

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

    /// Execute prefetch prediction after an access
    fn perform_prefetch(&mut self, accessed_key: &K) {
        self.prefetch_strategy.update_access_pattern(accessed_key);
        let predictions = self.prefetch_strategy.predict_next(accessed_key);

        for predicted_key in predictions {
            self.prefetch_stats.predictions_made += 1;
            if !self.t1_map.contains_key(&predicted_key)
                && !self.t2_map.contains_key(&predicted_key)
                    && !self.prefetch_buffer.contains_key(&predicted_key)
            {
                // Prediction stub (not actually loading entry)
            }
        }
        self.trim_prefetch_buffer();
    }

    fn trim_prefetch_buffer(&mut self) {
        while self.prefetch_buffer.len() > self.prefetch_buffer_size {
            if let Some(k) = self.prefetch_buffer.keys().next().cloned() {
                self.prefetch_buffer.remove(&k);
            } else {
                break;
            }
        }
    }

    /// Update adaptation parameter `p`
    fn update_p(&mut self, delta: i32) {
        if delta > 0 {
            self.p = (self.p + delta as usize).min(self.capacity);
        } else {
            self.p = self.p.saturating_sub((-delta) as usize);
        }
    }

    /// Advance T1 hand (Clock algorithm)
    fn advance_t1_hand(&mut self) -> Option<usize> {
        if self.t1_size == 0 {
            return None;
        }
        let start_pos = self.t1_hand;
        loop {
            let cur = self.t1_hand;
            self.t1_hand = (self.t1_hand + 1) % self.t1.len();

            if let Some(ref mut entry) = self.t1[cur] {
                if entry.reference_bit {
                    entry.reference_bit = false;
                } else {
                    return Some(cur);
                }
            }
            if self.t1_hand == start_pos {
                break;
            }
        }
        None
    }

    /// Advance T2 hand (Clock algorithm)
    fn advance_t2_hand(&mut self) -> Option<usize> {
        if self.t2_size == 0 {
            return None;
        }
        let start_pos = self.t2_hand;
        loop {
            let cur = self.t2_hand;
            self.t2_hand = (self.t2_hand + 1) % self.t2.len();

            if let Some(ref mut entry) = self.t2[cur] {
                if entry.reference_bit {
                    entry.reference_bit = false;
                } else {
                    return Some(cur);
                }
            }
            if self.t2_hand == start_pos {
                break;
            }
        }
        None
    }

    /// Find empty slots for T1 or T2
    fn find_empty_t1_slot(&self) -> Option<usize> {
        self.t1.iter().position(|s| s.is_none())
    }
    fn find_empty_t2_slot(&self) -> Option<usize> {
        self.t2.iter().position(|s| s.is_none())
    }

    /// Replacement procedure (eviction) for CAR
    fn replace(&mut self, in_b2: bool) -> bool {
        if self.t1_size >= 1 && ((in_b2 && self.t1_size == self.p) || self.t1_size > self.p) {
            if let Some(victim) = self.advance_t1_hand() {
                if let Some(entry) = self.t1[victim].take() {
                    self.t1_map.remove(&entry.key);
                    self.b1.insert(entry.key, ());
                    self.t1_size -= 1;
                    self.current_size -= 1;
                    return true;
                }
            }
        } else {
            if let Some(victim) = self.advance_t2_hand() {
                if let Some(entry) = self.t2[victim].take() {
                    self.t2_map.remove(&entry.key);
                    self.b2.insert(entry.key, ());
                    self.t2_size -= 1;
                    self.current_size -= 1;
                    return true;
                }
            }
        }
        false
    }

    /// Trim ghost buffers to at most capacity
    fn trim_ghost_buffers(&mut self) {
        let max = self.capacity;
        while self.b1.len() > max {
            if let Some(k) = self.b1.keys().next().cloned() {
                self.b1.remove(&k);
            } else {
                break;
            }
        }
        while self.b2.len() > max {
            if let Some(k) = self.b2.keys().next().cloned() {
                self.b2.remove(&k);
            } else {
                break;
            }
        }
    }
}

impl<K, V> CachePolicy<K, V> for CarCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn get(&mut self, key: &K) -> Option<&V> {
        // Prefetch buffer
        if let Some(_) = self.prefetch_buffer.get(key) {
            if let Some(val) = self.prefetch_buffer.remove(key) {
                self.prefetch_stats.cache_hits_from_prefetch += 1;
                self.insert(key.clone(), val);
                return self.get(key);
            }
        }

        // T1
        if let Some(&idx) = self.t1_map.get(key) {
            if let Some(entry) = self.t1[idx].take() {
                // Promote to T2
                self.t1_map.remove(key);
                self.t1_size -= 1;

                // Always allocate in T2
                let mut new_entry = CarEntry::new(entry.key.clone(), entry.value, ListType::T2);
                new_entry.reference_bit = true;

                if let Some(slot) = self.find_empty_t2_slot() {
                    self.t2[slot] = Some(new_entry);
                    self.t2_map.insert(key.clone(), slot);
                    self.t2_size += 1;
                    self.perform_prefetch(key);
                    return self.t2[slot].as_ref().map(|e| &e.value);
                } else if let Some(victim) = self.advance_t2_hand() {
                    if let Some(old) = self.t2[victim].take() {
                        self.t2_map.remove(&old.key);
                        self.b2.insert(old.key, ());
                        self.t2_size -= 1;
                        self.current_size -= 1;
                    }
                    self.t2[victim] = Some(new_entry);
                    self.t2_map.insert(key.clone(), victim);
                    self.t2_size += 1;
                    self.current_size += 1;
                    self.perform_prefetch(key);
                    return self.t2[victim].as_ref().map(|e| &e.value);
                }
            }
        }

        // T2
        if let Some(&idx) = self.t2_map.get(key) {
            // Borrow-scope trick to avoid conflict
            let value_ptr: *const V;
            {
                if let Some(ref mut entry) = self.t2[idx] {
                    entry.reference_bit = true;
                    value_ptr = &entry.value;
                } else {
                    return None;
                }
            }
            self.perform_prefetch(key);
            return Some(unsafe { &*value_ptr });
        }

        None
    }

    fn insert(&mut self, key: K, value: V) {
        self.prefetch_buffer.remove(&key);

        // Case 1: Already exists
        if let Some(&idx) = self.t1_map.get(&key) {
            if let Some(ref mut entry) = self.t1[idx] {
                entry.value = value;
                entry.reference_bit = true;
            }
            return;
        }
        if let Some(&idx) = self.t2_map.get(&key) {
            if let Some(ref mut entry) = self.t2[idx] {
                entry.value = value;
                entry.reference_bit = true;
            }
            return;
        }

        // Case 2: History hits (B1 or B2)
        if self.b1.contains_key(&key) {
            let delta = (self.b2.len() as f32 / self.b1.len().max(1) as f32).ceil() as i32;
            self.update_p(delta);
            if self.current_size >= self.capacity {
                self.replace(false);
            }
            self.b1.remove(&key);

            let new_entry = CarEntry::new(key.clone(), value, ListType::T2);
            if let Some(slot) = self.find_empty_t2_slot() {
                self.t2[slot] = Some(new_entry);
                self.t2_map.insert(key, slot);
                self.t2_size += 1;
                self.current_size += 1;
            }
            self.trim_ghost_buffers();
            return;
        }

        if self.b2.contains_key(&key) {
            let delta = (self.b1.len() as f32 / self.b2.len().max(1) as f32).ceil() as i32;
            self.update_p(-delta);
            if self.current_size >= self.capacity {
                self.replace(true);
            }
            self.b2.remove(&key);

            let new_entry = CarEntry::new(key.clone(), value, ListType::T2);
            if let Some(slot) = self.find_empty_t2_slot() {
                self.t2[slot] = Some(new_entry);
                self.t2_map.insert(key, slot);
                self.t2_size += 1;
                self.current_size += 1;
            }
            self.trim_ghost_buffers();
            return;
        }

        // Case 3: New entry
        let total_cache = self.t1_size + self.t2_size;
        if total_cache < self.capacity {
            if total_cache + self.b1.len() + self.b2.len() >= self.capacity {
                if self.b1.len() > self.b2.len() {
                    if let Some(k) = self.b1.keys().next().cloned() {
                        self.b1.remove(&k);
                    }
                } else if let Some(k) = self.b2.keys().next().cloned() {
                    self.b2.remove(&k);
                }
            }

            let new_entry = CarEntry::new(key.clone(), value, ListType::T1);
            if let Some(slot) = self.find_empty_t1_slot() {
                self.t1[slot] = Some(new_entry);
                self.t1_map.insert(key, slot);
                self.t1_size += 1;
                self.current_size += 1;
            }
        } else {
            self.replace(false);
            let new_entry = CarEntry::new(key.clone(), value, ListType::T1);
            if let Some(slot) = self.find_empty_t1_slot() {
                self.t1[slot] = Some(new_entry);
                self.t1_map.insert(key, slot);
                self.t1_size += 1;
                self.current_size += 1;
            }
        }
        self.trim_ghost_buffers();
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(val) = self.prefetch_buffer.remove(key) {
            return Some(val);
        }
        if let Some(idx) = self.t1_map.remove(key) {
            if let Some(entry) = self.t1[idx].take() {
                self.t1_size -= 1;
                self.current_size -= 1;
                return Some(entry.value);
            }
        }
        if let Some(idx) = self.t2_map.remove(key) {
            if let Some(entry) = self.t2[idx].take() {
                self.t2_size -= 1;
                self.current_size -= 1;
                return Some(entry.value);
            }
        }
        self.b1.remove(key);
        self.b2.remove(key);
        None
    }

    fn len(&self) -> usize {
        self.current_size
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn clear(&mut self) {
        for s in &mut self.t1 {
            *s = None;
        }
        for s in &mut self.t2 {
            *s = None;
        }
        self.t1_map.clear();
        self.t2_map.clear();
        self.b1.clear();
        self.b2.clear();
        self.t1_hand = 0;
        self.t2_hand = 0;
        self.t1_size = 0;
        self.t2_size = 0;
        self.current_size = 0;
        self.p = 0;
        self.prefetch_buffer.clear();
    }
}

impl<K, V> BenchmarkablePolicy<K, V> for CarCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn policy_type(&self) -> PolicyType {
        PolicyType::Car
    }

    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}_prefetch", self.policy_type().name(), self.capacity())
    }

    fn reset_for_benchmark(&mut self) {
        self.clear();
        self.reset_prefetch_stats();
    }
}

impl<K, V> Drop for CarCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

// Specialized constructors
impl CarCache<i32, String> {
    pub fn with_prefetch_i32(capacity: usize, prefetch_type: PrefetchType) -> Self {
        let strat = crate::prefetch::create_prefetch_strategy_i32(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}
impl CarCache<i64, String> {
    pub fn with_prefetch_i64(capacity: usize, prefetch_type: PrefetchType) -> Self {
        let strat = crate::prefetch::create_prefetch_strategy_i64(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}
impl CarCache<usize, String> {
    pub fn with_prefetch_usize(capacity: usize, prefetch_type: PrefetchType) -> Self {
        let strat = crate::prefetch::create_prefetch_strategy_usize(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}

unsafe impl<K, V> Send for CarCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{}
unsafe impl<K, V> Sync for CarCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{}

