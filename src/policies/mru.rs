use std::collections::HashMap;
use std::hash::Hash;
use std::ptr::NonNull;
use std::marker::PhantomData;
use crate::{CachePolicy, PrefetchStrategy};
use crate::prefetch::{PrefetchType, NoPrefetch};
use super::{BenchmarkablePolicy, PolicyType};

/// A Most Recently Used (MRU) cache implementation with integrated prefetch strategies
///
/// This cache maintains items in order of access, automatically evicting
/// the most recently used items when capacity is exceeded. It provides
/// O(1) average case performance for get, insert, and remove operations.
///
/// MRU is useful in scenarios where recently accessed items are less likely
/// to be accessed again, such as sequential scan patterns. The cache integrates
/// with prefetch strategies to predict and preload likely future accesses.
pub struct MruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// HashMap for O(1) key lookup
    map: HashMap<K, NonNull<Node<K, V>>>,
    /// Doubly-linked list for maintaining order
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
    /// Current number of items
    len: usize,
    /// Maximum capacity
    capacity: usize,
    /// Prefetch strategy for predicting future accesses
    prefetch_strategy: Box<dyn PrefetchStrategy<K>>,
    /// Prefetch buffer to store preloaded values
    prefetch_buffer: HashMap<K, V>,
    /// Maximum size of prefetch buffer
    prefetch_buffer_size: usize,
    /// Statistics for prefetch effectiveness
    prefetch_stats: PrefetchStats,
    _marker: PhantomData<Box<Node<K, V>>>,
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

/// Internal node structure for the doubly-linked list
struct Node<K, V> {
    key: K,
    value: V,
    prev: Option<NonNull<Node<K, V>>>,
    next: Option<NonNull<Node<K, V>>>,
}

impl<K, V> Node<K, V> {
    /// Creates a new node with the given key-value pair
    fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            prev: None,
            next: None,
        }
    }
}

impl<K, V> MruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new MRU cache with no prefetch (baseline)
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of items the cache can hold
    ///
    /// # Panics
    /// Panics if capacity is 0
    pub fn new(capacity: usize) -> Self {
        Self::with_custom_prefetch(capacity, Box::new(NoPrefetch))
    }

    /// Creates a new MRU cache with custom prefetch strategy
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
        assert!(capacity > 0, "MRU cache capacity must be greater than 0");

        Self {
            map: HashMap::new(),
            head: None,
            tail: None,
            len: 0,
            capacity,
            prefetch_strategy,
            prefetch_buffer: HashMap::new(),
            prefetch_buffer_size: (capacity / 4).max(1),
            prefetch_stats: PrefetchStats::default(),
            _marker: PhantomData,
        }
    }

    /// Creates a new MRU cache with default capacity of 100
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

    /// Moves the specified node to the front of the list (most recently used)
    ///
    /// # Safety
    /// The caller must ensure that node_ptr is a valid pointer to a node
    /// that exists in the current cache's linked list.
    unsafe fn move_to_front(&mut self, node_ptr: NonNull<Node<K, V>>) {
        let _node = unsafe { node_ptr.as_ref() };

        // If it's already at the front, nothing to do
        if self.head == Some(node_ptr) {
            return;
        }

        // Remove from current position
        unsafe { self.remove_from_list(node_ptr) };

        // Add to front
        unsafe { self.add_to_front(node_ptr) };
    }

    /// Removes a node from its current position in the linked list
    ///
    /// # Safety
    /// The caller must ensure that node_ptr is a valid pointer to a node
    /// that exists in the current cache's linked list.
    unsafe fn remove_from_list(&mut self, node_ptr: NonNull<Node<K, V>>) {
        let node = unsafe { node_ptr.as_ref() };

        // Update previous node's next pointer
        if let Some(mut prev) = node.prev {
            unsafe { prev.as_mut() }.next = node.next;
        } else {
            // This was the head
            self.head = node.next;
        }

        // Update next node's previous pointer
        if let Some(mut next) = node.next {
            unsafe { next.as_mut() }.prev = node.prev;
        } else {
            // This was the tail
            self.tail = node.prev;
        }
    }

    /// Adds a node to the front of the linked list
    ///
    /// # Safety
    /// The caller must ensure that node_ptr is a valid pointer to a node
    /// that is not currently in any linked list.
    unsafe fn add_to_front(&mut self, mut node_ptr: NonNull<Node<K, V>>) {
        let node = unsafe { node_ptr.as_mut() };
        node.prev = None;
        node.next = self.head;

        if let Some(mut old_head) = self.head {
            unsafe { old_head.as_mut() }.prev = Some(node_ptr);
        } else {
            // List was empty
            self.tail = Some(node_ptr);
        }

        self.head = Some(node_ptr);
    }

    /// Removes and deallocates the most recently used item (head)
    ///
    /// Returns the key of the evicted item, or None if the cache is empty.
    /// This is the key difference from LRU - we evict from head instead of tail.
    fn evict_mru(&mut self) -> Option<K> {
        if let Some(head_ptr) = self.head {
            unsafe {
                let head_node = Box::from_raw(head_ptr.as_ptr());
                let key = head_node.key.clone();

                // Remove from hash map
                self.map.remove(&key);

                // Update head pointer
                self.head = head_node.next;

                if let Some(mut new_head) = self.head {
                    new_head.as_mut().prev = None;
                } else {
                    // List is now empty
                    self.tail = None;
                }

                self.len -= 1;
                Some(key)
            }
        } else {
            None
        }
    }
}

// Specialized constructors for types that support our prefetch strategies
impl MruCache<i32, String> {
    /// Creates a new i32 MRU cache with specified prefetch strategy
    pub fn with_prefetch_i32(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "MRU cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_i32(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl MruCache<i64, String> {
    /// Creates a new i64 MRU cache with specified prefetch strategy
    pub fn with_prefetch_i64(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "MRU cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_i64(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl MruCache<usize, String> {
    /// Creates a new usize MRU cache with specified prefetch strategy
    pub fn with_prefetch_usize(capacity: usize, prefetch_type: PrefetchType) -> Self {
        assert!(capacity > 0, "MRU cache capacity must be greater than 0");
        let prefetch_strategy = crate::prefetch::create_prefetch_strategy_usize(prefetch_type);
        Self::with_custom_prefetch(capacity, prefetch_strategy)
    }
}

impl<K, V> CachePolicy<K, V> for MruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Retrieves a value from the cache and marks it as recently used
    ///
    /// Returns `Some(&V)` if the key exists, `None` otherwise.
    /// This operation moves the accessed item to the front of the MRU order
    /// and triggers prefetch predictions for future accesses.
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

        if let Some(&node_ptr) = self.map.get(key) {
            unsafe {
                // Move to front (mark as recently used)
                self.move_to_front(node_ptr);

                // Perform prefetch predictions
                self.perform_prefetch(key);

                Some(&node_ptr.as_ref().value)
            }
        } else {
            None
        }
    }

    /// Inserts a key-value pair into the cache
    ///
    /// If the key already exists, updates the value and moves it to front.
    /// If the cache is at capacity, evicts the most recently used item first.
    fn insert(&mut self, key: K, value: V) {
        // Remove from prefetch buffer if it exists there
        self.prefetch_buffer.remove(&key);

        // Check if key already exists
        if let Some(existing_ptr) = self.map.get_mut(&key) {
            let existing_ptr_value = *existing_ptr; // copy NonNull
            unsafe {
                (*existing_ptr_value.as_ptr()).value = value;
                self.move_to_front(existing_ptr_value);
            }
            return;
        }

        // Check if we need to evict before inserting
        if self.len >= self.capacity {
            self.evict_mru();
        }

        // Create new node
        let new_node = Box::new(Node::new(key.clone(), value));
        let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };

        // Add to hash map
        self.map.insert(key, node_ptr);

        // Add to front of list
        unsafe {
            self.add_to_front(node_ptr);
        }

        self.len += 1;
    }

    /// Removes an entry from the cache
    ///
    /// Returns the removed value if it existed, `None` otherwise.
    fn remove(&mut self, key: &K) -> Option<V> {
        // Check prefetch buffer first
        if let Some(value) = self.prefetch_buffer.remove(key) {
            return Some(value);
        }

        if let Some(node_ptr) = self.map.remove(key) {
            unsafe {
                // Remove from linked list
                self.remove_from_list(node_ptr);

                // Deallocate and extract value
                let node = Box::from_raw(node_ptr.as_ptr());
                self.len -= 1;

                Some(node.value)
            }
        } else {
            None
        }
    }

    /// Returns the current number of entries in the cache
    fn len(&self) -> usize {
        self.len
    }

    /// Removes all entries from the cache
    fn clear(&mut self) {
        // Deallocate all nodes
        while let Some(_) = self.evict_mru() {}

        // Reset state
        self.map.clear();
        self.head = None;
        self.tail = None;
        self.len = 0;
        self.prefetch_buffer.clear();
    }

    /// Returns the maximum capacity of the cache
    fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<K, V> BenchmarkablePolicy<K, V> for MruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Returns the policy type for this cache
    fn policy_type(&self) -> PolicyType {
        PolicyType::Mru
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

/// Safe wrapper that ensures proper cleanup
impl<K, V> Drop for MruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

// Implement Send and Sync if K and V are Send and Sync
unsafe impl<K, V> Send for MruCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}

unsafe impl<K, V> Sync for MruCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{
}

