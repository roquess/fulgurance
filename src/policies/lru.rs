use std::collections::HashMap;
use std::hash::Hash;
use std::ptr::NonNull;
use std::marker::PhantomData;
use crate::CachePolicy;
use super::{BenchmarkablePolicy, PolicyType};

impl<K, V> BenchmarkablePolicy<K, V> for LruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Returns the policy type for this cache
    fn policy_type(&self) -> PolicyType {
        PolicyType::Lru
    }
}

/// A Least Recently Used (LRU) cache implementation
/// 
/// This cache maintains items in order of access, automatically evicting
/// the least recently used items when capacity is exceeded. It provides
/// O(1) average case performance for get, insert, and remove operations.
pub struct LruCache<K, V> 
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
    _marker: PhantomData<Box<Node<K, V>>>,
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

impl<K, V> LruCache<K, V> 
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new LRU cache with the specified capacity
    /// 
    /// # Arguments
    /// * `capacity` - Maximum number of items the cache can hold
    /// 
    /// # Panics
    /// Panics if capacity is 0
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "LRU cache capacity must be greater than 0");
        
        Self {
            map: HashMap::new(),
            head: None,
            tail: None,
            len: 0,
            capacity,
            _marker: PhantomData,
        }
    }
    
    /// Creates a new LRU cache with default capacity of 100
    pub fn with_default_capacity() -> Self {
        Self::new(100)
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
    
    /// Removes and deallocates the least recently used item (tail)
    /// 
    /// Returns the key of the evicted item, or None if the cache is empty.
    fn evict_lru(&mut self) -> Option<K> {
        if let Some(tail_ptr) = self.tail {
            unsafe {
                let tail_node = Box::from_raw(tail_ptr.as_ptr());
                let key = tail_node.key.clone();
                
                // Remove from hash map
                self.map.remove(&key);
                
                // Update tail pointer
                self.tail = tail_node.prev;
                
                if let Some(mut new_tail) = self.tail {
                    new_tail.as_mut().next = None;
                } else {
                    // List is now empty
                    self.head = None;
                }
                
                self.len -= 1;
                Some(key)
            }
        } else {
            None
        }
    }
}

impl<K, V> CachePolicy<K, V> for LruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Retrieves a value from the cache and marks it as recently used
    /// 
    /// Returns `Some(&V)` if the key exists, `None` otherwise.
    /// This operation moves the accessed item to the front of the LRU order.
    fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(&node_ptr) = self.map.get(key) {
            unsafe {
                // Move to front (mark as recently used)
                self.move_to_front(node_ptr);
                Some(&node_ptr.as_ref().value)
            }
        } else {
            None
        }
    }
    
    /// Inserts a key-value pair into the cache
    /// 
    /// If the key already exists, updates the value and moves it to front.
    /// If the cache is at capacity, evicts the least recently used item first.
    fn insert(&mut self, key: K, value: V) {
        // Check if key already exists
        if let Some(existing_ptr) = self.map.get_mut(&key) {
            let existing_ptr_value = *existing_ptr; // copy NonNull
            unsafe {
                (*existing_ptr_value.as_ptr()).value = value;
                self.move_to_front(existing_ptr_value);
            }
            return;
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
        
        // Check if we need to evict
        if self.len > self.capacity {
            self.evict_lru();
        }
    }
    
    /// Removes an entry from the cache
    /// 
    /// Returns the removed value if it existed, `None` otherwise.
    fn remove(&mut self, key: &K) -> Option<V> {
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
        while let Some(_) = self.evict_lru() {}
        
        // Reset state
        self.map.clear();
        self.head = None;
        self.tail = None;
        self.len = 0;
    }
    
    /// Returns the maximum capacity of the cache
    fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Safe wrapper that ensures proper cleanup
impl<K, V> Drop for LruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

// Implement Send and Sync if K and V are Send and Sync
unsafe impl<K, V> Send for LruCache<K, V>
where
    K: Hash + Eq + Clone + Send,
    V: Clone + Send,
{
}

unsafe impl<K, V> Sync for LruCache<K, V>
where
    K: Hash + Eq + Clone + Sync,
    V: Clone + Sync,
{
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lru_basic_operations() {
        let mut cache = LruCache::new(3);
        
        // Test insertion
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(3, "three");
        
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&1), Some(&"one"));
        assert_eq!(cache.get(&2), Some(&"two"));
        assert_eq!(cache.get(&3), Some(&"three"));
    }
    
    #[test]
    fn test_lru_eviction() {
        let mut cache = LruCache::new(2);
        
        cache.insert(1, "one");
        cache.insert(2, "two");
        
        // This should evict key 1 (least recently used)
        cache.insert(3, "three");
        
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&1), None);  // Should be evicted
        assert_eq!(cache.get(&2), Some(&"two"));
        assert_eq!(cache.get(&3), Some(&"three"));
    }
    
    #[test]
    fn test_lru_access_order() {
        let mut cache = LruCache::new(3);
        
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(3, "three");
        
        // Access key 1 to make it most recently used
        cache.get(&1);
        
        // Insert new item, should evict key 2 (now least recently used)
        cache.insert(4, "four");
        
        assert_eq!(cache.get(&1), Some(&"one"));  // Should still be there
        assert_eq!(cache.get(&2), None);          // Should be evicted
        assert_eq!(cache.get(&3), Some(&"three"));
        assert_eq!(cache.get(&4), Some(&"four"));
    }
    
    #[test]
    fn test_lru_update_existing() {
        let mut cache = LruCache::new(2);
        
        cache.insert(1, "one");
        cache.insert(2, "two");
        
        // Update existing key
        cache.insert(1, "ONE");
        
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&1), Some(&"ONE"));
        assert_eq!(cache.get(&2), Some(&"two"));
    }
    
    #[test]
    fn test_lru_remove() {
        let mut cache = LruCache::new(3);
        
        cache.insert(1, "one");
        cache.insert(2, "two");
        
        assert_eq!(cache.remove(&1), Some("one"));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&"two"));
    }
    
    #[test]
    fn test_lru_clear() {
        let mut cache = LruCache::new(3);
        
        cache.insert(1, "one");
        cache.insert(2, "two");
        cache.insert(3, "three");
        
        cache.clear();
        
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.get(&1), None);
    }
    
    #[test]
    #[should_panic(expected = "LRU cache capacity must be greater than 0")]
    fn test_lru_zero_capacity_panics() {
        LruCache::<i32, String>::new(0);
    }
}
