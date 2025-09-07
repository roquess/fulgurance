use std::collections::HashMap;
use std::hash::Hash;
use std::ptr::NonNull;
use std::marker::PhantomData;
use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::{PrefetchType, NoPrefetch};
use super::{BenchmarkablePolicy, PolicyType};

/// Adaptive Replacement Cache (ARC) implementation with integrated prefetch strategies
///
/// ARC maintains four lists: T1 (recent), T2 (frequent), B1 (ghost T1), B2 (ghost T2)
/// It dynamically adjusts between recency and frequency based on access patterns
pub struct ArcCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// T1: Recent cache entries
    t1: HashMap<K, NonNull<Node<K, V>>>,
    /// T2: Frequent cache entries
    t2: HashMap<K, NonNull<Node<K, V>>>,
    /// B1: Ghost entries from T1 (keys only)
    b1: HashMap<K, ()>,
    /// B2: Ghost entries from T2 (keys only)
    b2: HashMap<K, ()>,
    
    /// Linked list heads and tails for each segment
    t1_head: Option<NonNull<Node<K, V>>>,
    t1_tail: Option<NonNull<Node<K, V>>>,
    t2_head: Option<NonNull<Node<K, V>>>,
    t2_tail: Option<NonNull<Node<K, V>>>,
    
    /// Adaptation parameter (target size of T1)
    p: usize,
    /// Total cache capacity
    capacity: usize,
    /// Current sizes
    t1_size: usize,
    t2_size: usize,
    
    /// Prefetch components (same as LRU)
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    prefetch_buffer: HashMap<K, V>,
    prefetch_buffer_size: usize,
    prefetch_stats: super::lru::PrefetchStats,
    
    _marker: PhantomData<Box<Node<K, V>>>,
}

/// Internal node structure for the doubly-linked lists
struct Node<K, V> {
    key: K,
    value: V,
    prev: Option<NonNull<Node<K, V>>>,
    next: Option<NonNull<Node<K, V>>>,
    /// Which list this node belongs to
    list_type: ListType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ListType {
    T1,
    T2,
}

impl<K, V> Node<K, V> {
    fn new(key: K, value: V, list_type: ListType) -> Self {
        Self {
            key,
            value,
            prev: None,
            next: None,
            list_type,
        }
    }
}

impl<K, V> ArcCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new ARC cache with no prefetch
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Creates a new ARC cache with custom prefetch strategy
    pub fn with_custom_prefetch(
        capacity: usize,
        prefetch_strategy: Box<dyn PrefetchStrategy<K>>
    ) -> Self {
        assert!(capacity > 0, "ARC cache capacity must be greater than 0");

        Self {
            t1: HashMap::new(),
            t2: HashMap::new(),
            b1: HashMap::new(),
            b2: HashMap::new(),
            t1_head: None,
            t1_tail: None,
            t2_head: None,
            t2_tail: None,
            p: 0,
            capacity,
            t1_size: 0,
            t2_size: 0,
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

    fn perform_prefetch(&mut self, accessed_key: &K) {
        self.prefetch_strategy.update_access_pattern(accessed_key);
        let predictions = self.prefetch_strategy.predict_next(accessed_key);

        for predicted_key in predictions {
            self.prefetch_stats.predictions_made += 1;
            if !self.t1.contains_key(&predicted_key) && 
               !self.t2.contains_key(&predicted_key) &&
               !self.prefetch_buffer.contains_key(&predicted_key) {
                // Prefetch prediction made but not loaded for demonstration
            }
        }
        
        self.trim_prefetch_buffer();
    }

    fn trim_prefetch_buffer(&mut self) {
        while self.prefetch_buffer.len() > self.prefetch_buffer_size {
            if let Some(key) = self.prefetch_buffer.keys().next().cloned() {
                self.prefetch_buffer.remove(&key);
            } else {
                break;
            }
        }
    }

    /// Add node to front of specified list
    unsafe fn add_to_front(&mut self, mut node_ptr: NonNull<Node<K, V>>, list_type: ListType) {
        let node = unsafe { node_ptr.as_mut() };
        node.list_type = list_type;
        node.prev = None;

        match list_type {
            ListType::T1 => {
                node.next = self.t1_head;
                if let Some(mut old_head) = self.t1_head {
                    unsafe { old_head.as_mut() }.prev = Some(node_ptr);
                } else {
                    self.t1_tail = Some(node_ptr);
                }
                self.t1_head = Some(node_ptr);
            }
            ListType::T2 => {
                node.next = self.t2_head;
                if let Some(mut old_head) = self.t2_head {
                    unsafe { old_head.as_mut() }.prev = Some(node_ptr);
                } else {
                    self.t2_tail = Some(node_ptr);
                }
                self.t2_head = Some(node_ptr);
            }
        }
    }

    /// Remove node from its current list
    unsafe fn remove_from_list(&mut self, node_ptr: NonNull<Node<K, V>>) {
        let node = unsafe { node_ptr.as_ref() };

        // Update previous node
        if let Some(mut prev) = node.prev {
            unsafe { prev.as_mut() }.next = node.next;
        } else {
            // This was the head
            match node.list_type {
                ListType::T1 => self.t1_head = node.next,
                ListType::T2 => self.t2_head = node.next,
            }
        }

        // Update next node
        if let Some(mut next) = node.next {
            unsafe { next.as_mut() }.prev = node.prev;
        } else {
            // This was the tail
            match node.list_type {
                ListType::T1 => self.t1_tail = node.prev,
                ListType::T2 => self.t2_tail = node.prev,
            }
        }
    }

    /// Replace operation for ARC algorithm
    fn replace(&mut self, in_b2: bool) {
        if self.t1_size >= 1 && 
           ((in_b2 && self.t1_size == self.p) || self.t1_size > self.p) {
            // Demote LRU page in T1 to B1
            if let Some(lru_ptr) = self.t1_tail {
                unsafe {
                    let lru_node = Box::from_raw(lru_ptr.as_ptr());
                    let key = lru_node.key.clone();
                    
                    self.t1.remove(&key);
                    self.b1.insert(key, ());
                    
                    self.t1_tail = lru_node.prev;
                    if let Some(mut new_tail) = self.t1_tail {
                        new_tail.as_mut().next = None;
                    } else {
                        self.t1_head = None;
                    }
                    self.t1_size -= 1;
                }
            }
        } else {
            // Demote LRU page in T2 to B2
            if let Some(lru_ptr) = self.t2_tail {
                unsafe {
                    let lru_node = Box::from_raw(lru_ptr.as_ptr());
                    let key = lru_node.key.clone();
                    
                    self.t2.remove(&key);
                    self.b2.insert(key, ());
                    
                    self.t2_tail = lru_node.prev;
                    if let Some(mut new_tail) = self.t2_tail {
                        new_tail.as_mut().next = None;
                    } else {
                        self.t2_head = None;
                    }
                    self.t2_size -= 1;
                }
            }
        }
    }

    /// Update adaptation parameter p
    fn update_p(&mut self, delta: i32) {
        if delta > 0 {
            self.p = (self.p + delta as usize).min(self.capacity);
        } else {
            self.p = self.p.saturating_sub((-delta) as usize);
        }
    }
}

impl<K, V> CachePolicy<K, V> for ArcCache<K, V>
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

        // Check T1
        if let Some(&node_ptr) = self.t1.get(key) {
            unsafe {
                // Move from T1 to T2 (promote to frequent)
                self.remove_from_list(node_ptr);
                self.t1.remove(key);
                self.t2.insert(key.clone(), node_ptr);
                self.add_to_front(node_ptr, ListType::T2);
                self.t1_size -= 1;
                self.t2_size += 1;

                self.perform_prefetch(key);
                return Some(&node_ptr.as_ref().value);
            }
        }

        // Check T2
        if let Some(&node_ptr) = self.t2.get(key) {
            unsafe {
                // Move to front of T2
                self.remove_from_list(node_ptr);
                self.add_to_front(node_ptr, ListType::T2);

                self.perform_prefetch(key);
                return Some(&node_ptr.as_ref().value);
            }
        }

        None
    }

    fn insert(&mut self, key: K, value: V) {
        // Remove from prefetch buffer if exists
        self.prefetch_buffer.remove(&key);

        // Case 1: x is in T1 or T2 (cache hit)
        if let Some(&node_ptr) = self.t1.get(&key).or(self.t2.get(&key)) {
            unsafe {
                (*node_ptr.as_ptr()).value = value;
                // This will be handled by get() call that typically follows
            }
            return;
        }

        // Case 2: x is in B1 (recent history hit)
        if self.b1.contains_key(&key) {
            // Adapt: increase p
            let delta = (self.b2.len() as f32 / self.b1.len() as f32).ceil() as i32;
            self.update_p(delta);
            
            // Replace
            self.replace(false);
            
            // Remove from B1 and add to T2
            self.b1.remove(&key);
            let new_node = Box::new(Node::new(key.clone(), value, ListType::T2));
            let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };
            
            self.t2.insert(key, node_ptr);
            unsafe { self.add_to_front(node_ptr, ListType::T2); }
            self.t2_size += 1;
            return;
        }

        // Case 3: x is in B2 (frequent history hit)
        if self.b2.contains_key(&key) {
            // Adapt: decrease p
            let delta = (self.b1.len() as f32 / self.b2.len() as f32).ceil() as i32;
            self.update_p(-delta);
            
            // Replace
            self.replace(true);
            
            // Remove from B2 and add to T2
            self.b2.remove(&key);
            let new_node = Box::new(Node::new(key.clone(), value, ListType::T2));
            let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };
            
            self.t2.insert(key, node_ptr);
            unsafe { self.add_to_front(node_ptr, ListType::T2); }
            self.t2_size += 1;
            return;
        }

        // Case 4: x is not in cache or history
        // Insert into T1
        let new_node = Box::new(Node::new(key.clone(), value, ListType::T1));
        let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };
        
        // Check if we need to make room
        let total_cache = self.t1_size + self.t2_size;
        let total_history = self.b1.len() + self.b2.len();
        
        if total_cache < self.capacity {
            // Cache not full
            if total_cache + total_history >= self.capacity {
                // Delete LRU page from B1 or B2
                if total_history >= self.capacity {
                    if self.b2.len() > 0 {
                        if let Some(key_to_remove) = self.b2.keys().next().cloned() {
                            self.b2.remove(&key_to_remove);
                        }
                    } else if let Some(key_to_remove) = self.b1.keys().next().cloned() {
                        self.b1.remove(&key_to_remove);
                    }
                }
            }
        } else {
            // Cache is full
            self.replace(false);
        }
        
        self.t1.insert(key, node_ptr);
        unsafe { self.add_to_front(node_ptr, ListType::T1); }
        self.t1_size += 1;
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        // Check prefetch buffer first
        if let Some(value) = self.prefetch_buffer.remove(key) {
            return Some(value);
        }

        // Check T1
        if let Some(node_ptr) = self.t1.remove(key) {
            unsafe {
                self.remove_from_list(node_ptr);
                let node = Box::from_raw(node_ptr.as_ptr());
                self.t1_size -= 1;
                return Some(node.value);
            }
        }

        // Check T2
        if let Some(node_ptr) = self.t2.remove(key) {
            unsafe {
                self.remove_from_list(node_ptr);
                let node = Box::from_raw(node_ptr.as_ptr());
                self.t2_size -= 1;
                return Some(node.value);
            }
        }

        // Remove from ghost lists
        self.b1.remove(key);
        self.b2.remove(key);

        None
    }

    fn len(&self) -> usize {
        self.t1_size + self.t2_size
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn clear(&mut self) {
        // Deallocate all T1 nodes
        while let Some(node_ptr) = self.t1_head {
            unsafe {
                let node = Box::from_raw(node_ptr.as_ptr());
                self.t1_head = node.next;
            }
        }

        // Deallocate all T2 nodes
        while let Some(node_ptr) = self.t2_head {
            unsafe {
                let node = Box::from_raw(node_ptr.as_ptr());
                self.t2_head = node.next;
            }
        }

        self.t1.clear();
        self.t2.clear();
        self.b1.clear();
        self.b2.clear();
        self.t1_head = None;
        self.t1_tail = None;
        self.t2_head = None;
        self.t2_tail = None;
        self.t1_size = 0;
        self.t2_size = 0;
        self.p = 0;
        self.prefetch_buffer.clear();
    }
}

impl<K, V> BenchmarkablePolicy<K, V> for ArcCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn policy_type(&self) -> PolicyType {
        PolicyType::Arc
    }

    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}_prefetch", self.policy_type().name(), self.capacity())
    }

    fn reset_for_benchmark(&mut self) {
        self.clear();
        self.reset_prefetch_stats();
    }
}

impl<K, V> Drop for ArcCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

// Specialized constructors for prefetch strategies
impl ArcCache<i32, String> {
    pub fn with_prefetch_i32(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "ARC cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_i32(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl ArcCache<i64, String> {
    pub fn with_prefetch_i64(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "ARC cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_i64(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl ArcCache<usize, String> {
    pub fn with_prefetch_usize(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "ARC cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_usize(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

unsafe impl<K, V> Send for ArcCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}

unsafe impl<K, V> Sync for ArcCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{
}
