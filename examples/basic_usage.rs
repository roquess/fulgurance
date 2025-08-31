use fulgurance::prelude::*;

fn main() {
    println!("=== Fulgurance Cache Library Demo ===\n");
    
    // Demo 1: Basic LRU cache usage
    demo_basic_lru();
    
    // Demo 2: Fulgurance with prefetching
    demo_fulgurance_prefetch();
    
    // Demo 3: Performance comparison
    demo_performance_comparison();
}

/// Demonstrates basic LRU cache functionality
fn demo_basic_lru() {
    println!("1. Basic LRU Cache Demo");
    println!("-----------------------");
    
    let mut cache = LruCache::new(3); // Capacity of 3
    
    // Insert some values
    cache.insert(1, "one");
    cache.insert(2, "two"); 
    cache.insert(3, "three");
    
    println!("After inserting 1,2,3:");
    println!("  Cache size: {}", cache.len());
    println!("  Get 1: {:?}", cache.get(&1));
    println!("  Get 2: {:?}", cache.get(&2));
    println!("  Get 3: {:?}", cache.get(&3));
    
    // Access key 1 to make it recently used
    cache.get(&1);
    println!("\nAfter accessing key 1:");
    
    // Insert key 4, should evict key 2 (least recently used)
    cache.insert(4, "four");
    println!("After inserting key 4:");
    println!("  Get 1: {:?}", cache.get(&1));
    println!("  Get 2: {:?} (should be evicted)", cache.get(&2));
    println!("  Get 3: {:?}", cache.get(&3));
    println!("  Get 4: {:?}", cache.get(&4));
    
    println!();
}

/// Demonstrates Fulgurance cache with prefetching
fn demo_fulgurance_prefetch() {
    println!("2. Fulgurance Cache with Prefetching Demo");
    println!("------------------------------------------");
    
    // Create cache and prefetch strategy
    let cache = LruCache::new(10);
    let prefetch = SequentialPrefetch::new();
    
    // Simulate a data source (like a database)
    let data_source = |key: &i32| -> Option<String> {
        if *key >= 0 && *key <= 100 {
            println!("    [DATA SOURCE] Loading key: {}", key);
            Some(format!("data_value_{}", key))
        } else {
            None
        }
    };
    
    let mut fulgrance = FulgranceCache::new(cache, prefetch)
        .with_prefetch_fn(data_source);
    
    println!("Accessing sequential data (should trigger prefetching):");
    
    // Access sequential keys - should trigger prefetching
    for i in 1..=5 {
        println!("  Requesting key {}:", i);
        if let Some(value) = fulgrance.get(&i) {
            println!("    Got: {}", value);
        }
        println!("    Cache size after access: {}", fulgrance.len());
    }
    
    println!("\nCache statistics:");
    let stats = fulgrance.stats();
    println!("  Total accesses: {}", stats.total_accesses);
    println!("  Cache hits: {}", stats.hits);
    println!("  Cache misses: {}", stats.misses);
    println!("  Hit ratio: {:.2}%", stats.hit_ratio() * 100.0);
    println!("  Prefetch hits: {}", stats.prefetch_hits);
    
    println!();
}

/// Demonstrates performance comparison between different configurations
fn demo_performance_comparison() {
    println!("3. Performance Comparison Demo");
    println!("------------------------------");
    
    let test_size = 1000;
    let cache_size = 100;
    
    // Test data: sequential access pattern
    let access_pattern: Vec<i32> = (1..=test_size).collect();
    
    // Test 1: LRU only (no prefetching)
    println!("Testing LRU without prefetching...");
    let start = std::time::Instant::now();
    let mut cache_only = LruCache::new(cache_size);
    let mut hits = 0;
    
    for &key in &access_pattern {
        if cache_only.get(&key).is_some() {
            hits += 1;
        } else {
            cache_only.insert(key, format!("value_{}", key));
        }
    }
    let duration_no_prefetch = start.elapsed();
    let hit_ratio_no_prefetch = hits as f64 / test_size as f64;
    
    // Test 2: Fulgurance with sequential prefetching
    println!("Testing Fulgurance with sequential prefetching...");
    let start = std::time::Instant::now();
    let cache = LruCache::new(cache_size);
    let prefetch = SequentialPrefetch::new();
    let mut fulgrance = FulgranceCache::new(cache, prefetch)
        .with_prefetch_fn(|key: &i32| Some(format!("value_{}", key)));
    
    for &key in &access_pattern {
        fulgrance.get(&key);
    }
    let duration_with_prefetch = start.elapsed();
    let stats = fulgrance.stats();
    
    // Results
    println!("\nResults:");
    println!("  LRU only:");
    println!("    Time: {:?}", duration_no_prefetch);
    println!("    Hit ratio: {:.2}%", hit_ratio_no_prefetch * 100.0);
    println!("  Fulgurance with prefetching:");
    println!("    Time: {:?}", duration_with_prefetch);
    println!("    Hit ratio: {:.2}%", stats.hit_ratio() * 100.0);
    println!("    Total accesses: {}", stats.total_accesses);
    println!("    Cache hits: {}", stats.hits);
    println!("    Prefetch hits: {}", stats.prefetch_hits);
    
    let speedup = duration_no_prefetch.as_nanos() as f64 / duration_with_prefetch.as_nanos() as f64;
    if speedup > 1.0 {
        println!("    Speedup: {:.2}x faster", speedup);
    } else {
        println!("    Overhead: {:.2}x slower", 1.0 / speedup);
    }
}
