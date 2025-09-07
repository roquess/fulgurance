use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::ptr::NonNull;
use std::marker::PhantomData;
use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::{PrefetchType, NoPrefetch};
use super::{BenchmarkablePolicy, PolicyType};

/// 2Q cache implementation with integrated prefetch strategies
///
/// 2Q uses three lists:
/// - A1 (FIFO): Recently referenced pages (first time access)
/// - Am (LRU): Frequently referenced pages (promoted from A1out)
/// - A1out (FIFO): Ghost buffer of recently evicted A1 pages
pub struct TwoQCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// FIFO queue for recently referenced pages (first access)
    a1: VecDeque<K>,
    /// Hash map for A1 entries (for O(1) lookup)
    a1_map: HashMap<K, NonNull<Node<K, V>>>,
    
    /// LRU list for frequently referenced pages
    am_map: HashMap<K, NonNull<Node<K, V>>>,
    am_head: Option<NonNull<Node<K, V>>>,
    am_tail: Option<NonNull<Node<K, V>>>,
    
    /// FIFO ghost buffer of evicted A1 pages (keys only)
    a1out: VecDeque<K>,
    
    /// Size parameters
    capacity: usize,
    a1_capacity: usize,      // Kin (typically capacity/4)
    a1out_capacity: usize,   // Kout (typically capacity/2)
    am_capacity: usize,      // capacity - a1_capacity
    
    /// Current sizes
    a1_size: usize,
    am_size: usize,
    
    /// Prefetch components
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    prefetch_buffer: HashMap<K, V>,
    prefetch_buffer_size: usize,
    prefetch_stats: super::lru::PrefetchStats,
    
    _marker: PhantomData<Box<Node<K, V>>>,
}

/// Internal node structure for Am (LRU) list
struct Node<K, V> {
    key: K,
    value: V,
    prev: Option<NonNull<Node<K, V>>>,
    next: Option<NonNull<Node<K, V>>>,
}

impl<K, V> Node<K, V> {
    fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            prev: None,
            next: None,
        }
    }
}

impl<K, V> TwoQCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new 2Q cache with no prefetch
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Creates a new 2Q cache with custom prefetch strategy
    pub fn with_custom_prefetch(
        capacity: usize,
        prefetch_strategy: Box<dyn PrefetchStrategy<K>>
    ) -> Self {
        assert!(capacity > 0, "2Q cache capacity must be greater than 0");

        // Standard 2Q parameters
        let a1_capacity = (capacity / 4).max(1);
        let a1out_capacity = (capacity / 2).max(1);
        let am_capacity = capacity - a1_capacity;

        Self {
            a1: VecDeque::new(),
            a1_map: HashMap::new(),
            am_map: HashMap::new(),
            am_head: None,
            am_tail: None,
            a1out: VecDeque::new(),
            capacity,
            a1_capacity,
            a1out_capacity,
            am_capacity,
            a1_size: 0,
            am_size: 0,
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
            if !self.a1_map.contains_key(&predicted_key) && 
               !self.am_map.contains_key(&predicted_key) &&
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

    /// Moves node to front of Am list
    unsafe fn move_am_to_front(&mut self, node_ptr: NonNull<Node<K, V>>) {
        if self.am_head == Some(node_ptr) {
            return;
        }

        // Remove from current position
        unsafe { self.remove_am_from_list(node_ptr) };

        // Add to front
        unsafe { self.add_am_to_front(node_ptr) };
    }

    /// Removes node from Am list
    unsafe fn remove_am_from_list(&mut self, node_ptr: NonNull<Node<K, V>>) {
        let node = unsafe { node_ptr.as_ref() };

        if let Some(mut prev) = node.prev {
            unsafe { prev.as_mut() }.next = node.next;
        } else {
            self.am_head = node.next;
        }

        if let Some(mut next) = node.next {
            unsafe { next.as_mut() }.prev = node.prev;
        } else {
            self.am_tail = node.prev;
        }
    }

    /// Adds node to front of Am list
    unsafe fn add_am_to_front(&mut self, mut node_ptr: NonNull<Node<K, V>>) {
        let node = unsafe { node_ptr.as_mut() };
        node.prev = None;
        node.next = self.am_head;

        if let Some(mut old_head) = self.am_head {
            unsafe { old_head.as_mut() }.prev = Some(node_ptr);
        } else {
            self.am_tail = Some(node_ptr);
        }

        self.am_head = Some(node_ptr);
    }

    /// Evicts LRU item from Am
    fn evict_am_lru(&mut self) -> Option<K> {
        if let Some(tail_ptr) = self.am_tail {
            unsafe {
                let tail_node = Box::from_raw(tail_ptr.as_ptr());
                let key = tail_node.key.clone();

                self.am_map.remove(&key);
                self.am_tail = tail_node.prev;

                if let Some(mut new_tail) = self.am_tail {
                    new_tail.as_mut().next = None;
                } else {
                    self.am_head = None;
                }

                self.am_size -= 1;
                Some(key)
            }
        } else {
            None
        }
    }

    /// Reclaim space by evicting from A1 and possibly Am
    fn reclaim(&mut self) {
        // First try to evict from A1
        if self.a1_size >= self.a1_capacity {
            if let Some(evicted_key) = self.a1.pop_front() {
                if let Some(node_ptr) = self.a1_map.remove(&evicted_key) {
                    unsafe {
                        let _node = Box::from_raw(node_ptr.as_ptr());
                    }
                    self.a1_size -= 1;

                    // Add to A1out
                    self.a1out.push_back(evicted_key);
                    if self.a1out.len() > self.a1out_capacity {
                        self.a1out.pop_front();
                    }
                }
            }
        }

        // If Am is over capacity, evict from Am
        if self.am_size >= self.am_capacity {
            self.evict_am_lru();
        }
    }
}

impl<K, V> CachePolicy<K, V> for TwoQCache<K, V>
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

        // Check A1 first
        if let Some(&node_ptr) = self.a1_map.get(key) {
            unsafe {
                self.perform_prefetch(key);
                return Some(&node_ptr.as_ref().value);
            }
        }

        // Check Am
        if let Some(&node_ptr) = self.am_map.get(key) {
            unsafe {
                // Move to front of Am (LRU update)
                self.move_am_to_front(node_ptr);
                self.perform_prefetch(key);
                return Some(&node_ptr.as_ref().value);
            }
        }

        None
    }

    fn insert(&mut self, key: K, value: V) {
        // Remove from prefetch buffer if exists
        self.prefetch_buffer.remove(&key);

        // Case 1: Page is in A1
        if let Some(&node_ptr) = self.a1_map.get(&key) {
            unsafe {
                (*node_ptr.as_ptr()).value = value;
                return;
            }
        }

        // Case 2: Page is in Am  
        if let Some(&node_ptr) = self.am_map.get(&key) {
            unsafe {
                (*node_ptr.as_ptr()).value = value;
                self.move_am_to_front(node_ptr);
                return;
            }
        }

        // Case 3: Page is in A1out (promote to Am)
        if let Some(pos) = self.a1out.iter().position(|x| x == &key) {
            // Remove from A1out
            self.a1out.remove(pos);

            // Make room in Am if necessary
            if self.am_size >= self.am_capacity {
                self.evict_am_lru();
            }

            // Add to Am
            let new_node = Box::new(Node::new(key.clone(), value));
            let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };
            
            self.am_map.insert(key, node_ptr);
            unsafe { self.add_am_to_front(node_ptr); }
            self.am_size += 1;
            return;
        }

        // Case 4: Page is not in cache (add to A1)
        // Make room if necessary
        self.reclaim();

        let new_node = Box::new(Node::new(key.clone(), value));
        let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };
        
        self.a1_map.insert(key.clone(), node_ptr);
        self.a1.push_back(key);
        self.a1_size += 1;
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        // Check prefetch buffer first
        if let Some(value) = self.prefetch_buffer.remove(key) {
            return Some(value);
        }

        // Remove from A1
        if let Some(node_ptr) = self.a1_map.remove(key) {
            unsafe {
                let node = Box::from_raw(node_ptr.as_ptr());
                
                // Remove from A1 queue
                if let Some(pos) = self.a1.iter().position(|x| x == key) {
                    self.a1.remove(pos);
                }
                
                self.a1_size -= 1;
                return Some(node.value);
            }
        }

        // Remove from Am
        if let Some(node_ptr) = self.am_map.remove(key) {
            unsafe {
                self.remove_am_from_list(node_ptr);
                let node = Box::from_raw(node_ptr.as_ptr());
                self.am_size -= 1;
                return Some(node.value);
            }
        }

        // Remove from A1out
        if let Some(pos) = self.a1out.iter().position(|x| x == key) {
            self.a1out.remove(pos);
        }

        None
    }

    fn len(&self) -> usize {
        self.a1_size + self.am_size
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn clear(&mut self) {
        // Deallocate all A1 nodes
        for (_, node_ptr) in self.a1_map.drain() {
            unsafe {
                let _node = Box::from_raw(node_ptr.as_ptr());
            }
        }

        // Deallocate all Am nodes
        for (_, node_ptr) in self.am_map.drain() {
            unsafe {
                let _node = Box::from_raw(node_ptr.as_ptr());
            }
        }

        self.a1.clear();
        self.a1out.clear();
        self.am_head = None;
        self.am_tail = None;
        self.a1_size = 0;
        self.am_size = 0;
        self.prefetch_buffer.clear();
    }
}

impl<K, V> BenchmarkablePolicy<K, V> for TwoQCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn policy_type(&self) -> PolicyType {
        PolicyType::TwoQ
    }

    fn benchmark_name(&self) -> String {
        format!("{}_cap_{}_prefetch", self.policy_type().name(), self.capacity())
    }

    fn reset_for_benchmark(&mut self) {
        self.clear();
        self.reset_prefetch_stats();
    }
}

impl<K, V> Drop for TwoQCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

// Specialized constructors for prefetch strategies
impl TwoQCache<i32, String> {
    pub fn with_prefetch_i32(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "2Q cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_i32(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl TwoQCache<i64, String> {
    pub fn with_prefetch_i64(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "2Q cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_i64(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl TwoQCache<usize, String> {
    pub fn with_prefetch_usize(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "2Q cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_usize(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

unsafe impl<K, V> Send for TwoQCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}

unsafe impl<K, V> Sync for TwoQCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{
}
