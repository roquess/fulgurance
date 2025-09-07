use std::collections::HashMap;
use std::hash::Hash;
use std::ptr::NonNull;
use std::marker::PhantomData;

use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::{PrefetchType, NoPrefetch};
use super::{BenchmarkablePolicy, PolicyType};

/// Segmented LRU (SLRU) cache implementation with prefetching support
/// 
/// Splits the cache into two segments:
/// - Probationary segment holds newly inserted entries
/// - Protected segment holds frequently accessed entries
/// 
/// This design protects frequent items while evicting one-time accesses quickly.
pub struct SlruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Probationary segment: map from key to intrusive node pointer
    probationary_map: HashMap<K, NonNull<Node<K, V>>>,
    probationary_head: Option<NonNull<Node<K, V>>>,
    probationary_tail: Option<NonNull<Node<K, V>>>,
    probationary_size: usize,
    probationary_capacity: usize,

    /// Protected segment: map from key to intrusive node pointer
    protected_map: HashMap<K, NonNull<Node<K, V>>>,
    protected_head: Option<NonNull<Node<K, V>>>,
    protected_tail: Option<NonNull<Node<K, V>>>,
    protected_size: usize,
    protected_capacity: usize,

    /// Total capacity for the entire cache
    capacity: usize,

    /// Prefetch strategy and buffer
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    prefetch_buffer: HashMap<K, V>,
    prefetch_buffer_size: usize,
    prefetch_stats: super::lru::PrefetchStats,

    /// PhantomData for ownership tracking of Nodes
    _marker: PhantomData<Box<Node<K, V>>>,
}

/// Internal cache node type used in SLRU doubly linked lists
struct Node<K, V> {
    key: K,
    value: V,
    prev: Option<NonNull<Node<K, V>>>,
    next: Option<NonNull<Node<K, V>>>,
    segment: Segment,
}

/// Represents one of the two SLRU segments
#[derive(Debug, Clone, Copy, PartialEq)]
enum Segment {
    Probationary,
    Protected,
}

impl<K, V> Node<K, V> {
    /// Construct a new node belonging to a specific segment
    fn new(key: K, value: V, segment: Segment) -> Self {
        Node {
            key,
            value,
            prev: None,
            next: None,
            segment,
        }
    }
}

impl<K, V> SlruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new SLRU cache with no prefetch strategy (default 80/20 split)
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Create with specified prefetch strategy and 80/20 segment split
    pub fn with_custom_prefetch(
        capacity: usize,
        prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    ) -> Self {
        assert!(capacity > 0, "SLRU cache capacity must be > 0");

        let protected_capacity = ((capacity as f64) * 0.8).ceil() as usize;
        let probationary_capacity = capacity - protected_capacity;

        SlruCache {
            probationary_map: HashMap::new(),
            probationary_head: None,
            probationary_tail: None,
            probationary_size: 0,
            probationary_capacity,

            protected_map: HashMap::new(),
            protected_head: None,
            protected_tail: None,
            protected_size: 0,
            protected_capacity,

            capacity,
            prefetch_strategy,
            prefetch_buffer: HashMap::new(),
            prefetch_buffer_size: (capacity / 4).max(1),
            prefetch_stats: super::lru::PrefetchStats::default(),

            _marker: PhantomData,
        }
    }

    /// Access prefetch statistics
    pub fn prefetch_stats(&self) -> &super::lru::PrefetchStats {
        &self.prefetch_stats
    }

    /// Reset prefetch statistics and strategy
    pub fn reset_prefetch_stats(&mut self) {
        self.prefetch_stats = super::lru::PrefetchStats::default();
        self.prefetch_strategy.reset();
    }

    /// Perform prefetch update after key access
    fn perform_prefetch(&mut self, accessed_key: &K) {
        self.prefetch_strategy.update_access_pattern(accessed_key);
        let predictions = self.prefetch_strategy.predict_next(accessed_key);

        for predicted_key in predictions {
            self.prefetch_stats.predictions_made += 1;

            if !self.probationary_map.contains_key(&predicted_key)
                && !self.protected_map.contains_key(&predicted_key)
                && !self.prefetch_buffer.contains_key(&predicted_key)
            {
                // Prefetch prediction (not actually loading in this demo)
            }
        }
        self.trim_prefetch_buffer();
    }

    /// Trim the prefetch buffer to maintain max size
    fn trim_prefetch_buffer(&mut self) {
        while self.prefetch_buffer.len() > self.prefetch_buffer_size {
            if let Some(key) = self.prefetch_buffer.keys().next().cloned() {
                self.prefetch_buffer.remove(&key);
            } else {
                break;
            }
        }
    }

    /// Move the node to the front of its segment list
    unsafe fn move_to_front(&mut self, node_ptr: NonNull<Node<K, V>>, segment: Segment) {
        let current_segment = unsafe { node_ptr.as_ref() }.segment;

        if current_segment != segment {
            unsafe { self.remove_from_list(node_ptr) };
            unsafe { self.add_to_front(node_ptr, segment) };
        } else {
            match segment {
                Segment::Probationary => {
                    if self.probationary_head == Some(node_ptr) {
                        return;
                    }
                }
                Segment::Protected => {
                    if self.protected_head == Some(node_ptr) {
                        return;
                    }
                }
            }
            unsafe { self.remove_from_list(node_ptr) };
            unsafe { self.add_to_front(node_ptr, segment) };
        }
    }

    /// Remove the node from its current list
    unsafe fn remove_from_list(&mut self, node_ptr: NonNull<Node<K, V>>) {
        let node = unsafe { node_ptr.as_ref() };

        // Update previous node's next pointer
        if let Some(mut prev) = node.prev {
            unsafe { prev.as_mut() }.next = node.next;
        } else {
            // Node is head of its segment
            match node.segment {
                Segment::Probationary => self.probationary_head = node.next,
                Segment::Protected => self.protected_head = node.next,
            }
        }

        // Update next node's previous pointer
        if let Some(mut next) = node.next {
            unsafe { next.as_mut() }.prev = node.prev;
        } else {
            // Node is tail of its segment
            match node.segment {
                Segment::Probationary => self.probationary_tail = node.prev,
                Segment::Protected => self.protected_tail = node.prev,
            }
        }
    }

    /// Add the node to the front of the specified segment list
    unsafe fn add_to_front(&mut self, mut node_ptr: NonNull<Node<K, V>>, segment: Segment) {
        let node = unsafe { node_ptr.as_mut() };
        node.segment = segment;
        node.prev = None;

        match segment {
            Segment::Probationary => {
                node.next = self.probationary_head;
                if let Some(mut old_head) = self.probationary_head {
                    unsafe { old_head.as_mut() }.prev = Some(node_ptr);
                } else {
                    self.probationary_tail = Some(node_ptr);
                }
                self.probationary_head = Some(node_ptr);
            }
            Segment::Protected => {
                node.next = self.protected_head;
                if let Some(mut old_head) = self.protected_head {
                    unsafe { old_head.as_mut() }.prev = Some(node_ptr);
                } else {
                    self.protected_tail = Some(node_ptr);
                }
                self.protected_head = Some(node_ptr);
            }
        }
    }

    /// Evict least recently used node from probationary segment
    fn evict_probationary_lru(&mut self) -> Option<K> {
        if let Some(tail_ptr) = self.probationary_tail {
            unsafe {
                let tail_node = Box::from_raw(tail_ptr.as_ptr());
                let key = tail_node.key.clone();

                self.probationary_map.remove(&key);
                self.probationary_tail = tail_node.prev;

                if let Some(mut new_tail) = self.probationary_tail {
                    new_tail.as_mut().next = None;
                } else {
                    self.probationary_head = None;
                }

                self.probationary_size -= 1;

                Some(key)
            }
        } else {
            None
        }
    }

    /// Evict least recently used node from protected segment
    fn evict_protected_lru(&mut self) -> Option<K> {
        if let Some(tail_ptr) = self.protected_tail {
            unsafe {
                let tail_node = Box::from_raw(tail_ptr.as_ptr());
                let key = tail_node.key.clone();

                self.protected_map.remove(&key);
                self.protected_tail = tail_node.prev;

                if let Some(mut new_tail) = self.protected_tail {
                    new_tail.as_mut().next = None;
                } else {
                    self.protected_head = None;
                }

                self.protected_size -= 1;

                Some(key)
            }
        } else {
            None
        }
    }

    /// Promote a node from probationary to protected segment
    unsafe fn promote_to_protected(&mut self, key: &K) -> bool {
        if let Some(node_ptr) = self.probationary_map.remove(key) {
            if self.protected_size >= self.protected_capacity {
                self.evict_protected_lru();
            }

            unsafe { self.remove_from_list(node_ptr) };
            self.probationary_size -= 1;

            self.protected_map.insert(key.clone(), node_ptr);
            unsafe { self.add_to_front(node_ptr, Segment::Protected) };
            self.protected_size += 1;

            true
        } else {
            false
        }
    }
}

impl<K, V> CachePolicy<K, V> for SlruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Retrieve value associated with key, promoting if needed
    fn get(&mut self, key: &K) -> Option<&V> {
        // Check prefetch buffer first
        if let Some(_) = self.prefetch_buffer.get(key) {
            if let Some(value) = self.prefetch_buffer.remove(key) {
                self.prefetch_stats.cache_hits_from_prefetch += 1;
                self.insert(key.clone(), value);
                return self.get(key);
            }
        }

        // Check probationary segment (promote on hit)
        if let Some(&node_ptr) = self.probationary_map.get(key) {
            unsafe {
                self.promote_to_protected(key);
                self.perform_prefetch(key);
                return Some(&node_ptr.as_ref().value);
            }
        }

        // Check protected segment (move to front on hit)
        if let Some(&node_ptr) = self.protected_map.get(key) {
            unsafe {
                self.move_to_front(node_ptr, Segment::Protected);
                self.perform_prefetch(key);
                return Some(&node_ptr.as_ref().value);
            }
        }

        // Not found in cache
        None
    }

    /// Insert or update cache entry
    fn insert(&mut self, key: K, value: V) {
        self.prefetch_buffer.remove(&key);

        // Update if exists in probationary segment
        if let Some(&node_ptr) = self.probationary_map.get(&key) {
            unsafe {
                (*node_ptr.as_ptr()).value = value;
            }
            return;
        }

        // Update if exists in protected segment
        if let Some(&node_ptr) = self.protected_map.get(&key) {
            unsafe {
                (*node_ptr.as_ptr()).value = value;
                self.move_to_front(node_ptr, Segment::Protected);
            }
            return;
        }

        // Insert new node into probationary segment

        // Evict LRU if probationary segment full
        if self.probationary_size >= self.probationary_capacity {
            self.evict_probationary_lru();
        }

        let new_node = Box::new(Node::new(key.clone(), value, Segment::Probationary));
        let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };

        self.probationary_map.insert(key, node_ptr);
        unsafe { self.add_to_front(node_ptr, Segment::Probationary) };
        self.probationary_size += 1;
    }

    /// Remove entry from cache if present
    fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(value) = self.prefetch_buffer.remove(key) {
            return Some(value);
        }

        if let Some(node_ptr) = self.probationary_map.remove(key) {
            unsafe {
                self.remove_from_list(node_ptr);
                let node = Box::from_raw(node_ptr.as_ptr());
                self.probationary_size -= 1;
                return Some(node.value);
            }
        }

        if let Some(node_ptr) = self.protected_map.remove(key) {
            unsafe {
                self.remove_from_list(node_ptr);
                let node = Box::from_raw(node_ptr.as_ptr());
                self.protected_size -= 1;
                return Some(node.value);
            }
        }

        None
    }

    /// Current total number of cached entries
    fn len(&self) -> usize {
        self.probationary_size + self.protected_size
    }

    /// Maximum entries allowed in cache
    fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears all cache entries and frees memory
    fn clear(&mut self) {
        for (_, node_ptr) in self.probationary_map.drain() {
            unsafe {
                let _ = Box::from_raw(node_ptr.as_ptr());
            }
        }
        for (_, node_ptr) in self.protected_map.drain() {
            unsafe {
                let _ = Box::from_raw(node_ptr.as_ptr());
            }
        }

        self.probationary_head = None;
        self.probationary_tail = None;
        self.protected_head = None;
        self.protected_tail = None;
        self.probationary_size = 0;
        self.protected_size = 0;

        self.prefetch_buffer.clear();
    }
}

impl<K, V> BenchmarkablePolicy<K, V> for SlruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn policy_type(&self) -> PolicyType {
        PolicyType::Slru
    }

    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}_prefetch", self.policy_type().name(), self.capacity())
    }

    fn reset_for_benchmark(&mut self) {
        self.clear();
        self.reset_prefetch_stats();
    }
}

impl<K, V> Drop for SlruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

/// Specialized constructors for convenience

impl SlruCache<i32, String> {
    pub fn with_prefetch_i32(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "SLRU cache capacity must be greater than 0");
        let strat = crate::prefetch::create_prefetch_strategy_i32(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}

impl SlruCache<i64, String> {
    pub fn with_prefetch_i64(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "SLRU cache capacity must be greater than 0");
        let strat = crate::prefetch::create_prefetch_strategy_i64(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}

impl SlruCache<usize, String> {
    pub fn with_prefetch_usize(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "SLRU cache capacity must be greater than 0");
        let strat = crate::prefetch::create_prefetch_strategy_usize(prefetch_type);
        Self::with_custom_prefetch(capacity, strat)
    }
}

unsafe impl<K, V> Send for SlruCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}

unsafe impl<K, V> Sync for SlruCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{
}

