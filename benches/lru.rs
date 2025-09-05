use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::prefetch::PrefetchType;

/// Helper function to create cache with specified prefetch strategy
fn create_lru_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> LruCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => LruCache::new(capacity),
        _ => LruCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Benchmarks basic LRU cache operations across all prefetch strategies
fn bench_lru_basic_operations(c: &mut Criterion) {
    println!("Starting bench_lru_basic_operations...");
    let mut group = c.benchmark_group("lru_basic");

    let cache_sizes = vec![100, 500, 1000];
    let operation_counts = vec![1000, 5000];

    for &cache_size in &cache_sizes {
        for &op_count in &operation_counts {
            // Test each prefetch strategy
            for &prefetch_type in PrefetchType::all() {
                let strategy_name = prefetch_type.name();
                
                group.bench_with_input(
                    BenchmarkId::new(
                        format!("{}_insert_get", strategy_name),
                        format!("cap{}_ops{}", cache_size, op_count)
                    ),
                    &(cache_size, op_count, prefetch_type),
                    |b, &(cache_size, op_count, prefetch_type)| {
                        b.iter(|| {
                            let mut cache = create_lru_cache_with_prefetch(cache_size, prefetch_type);

                            // Insert operations
                            for i in 0..op_count {
                                cache.insert(i as i32, format!("value_{}", i));
                            }

                            // Get operations
                            for i in 0..op_count {
                                let _ = cache.get(&(i as i32));
                            }

                            cache.len()
                        })
                    },
                );
            }
        }
    }

    group.finish();
}

/// Benchmarks LRU cache with different access patterns across all prefetch strategies
fn bench_lru_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_access_patterns");

    let cache_size = 200;
    let data_size = 1000;

    // Define access patterns with their expected beneficiaries
    let patterns = vec![
        ("sequential", "Sequential prefetch should excel here"),
        ("random", "No prefetch strategy should have clear advantage"),
        ("working_set", "Markov might adapt better to hotspot patterns"),
    ];

    for (pattern_name, _description) in patterns {
        for &prefetch_type in PrefetchType::all() {
            let strategy_name = prefetch_type.name();
            
            group.bench_function(
                format!("{}_{}", strategy_name, pattern_name),
                |b| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, prefetch_type);

                        match pattern_name {
                            "sequential" => {
                                // Sequential access pattern - should benefit Sequential prefetch
                                for i in 0..data_size {
                                    let key = i as i32;
                                    if cache.get(&key).is_none() {
                                        cache.insert(key, format!("value_{}", key));
                                    }
                                }
                            }
                            "random" => {
                                // Random access pattern (simplified)
                                for i in 0..data_size {
                                    let key = (i * 17) % (data_size * 2); // Simple pseudo-random
                                    let key = key as i32;
                                    if cache.get(&key).is_none() {
                                        cache.insert(key, format!("value_{}", key));
                                    }
                                }
                            }
                            "working_set" => {
                                // Working set pattern (80/20 rule) - might benefit Markov
                                let hot_keys = data_size / 5; // 20% of keys are "hot"
                                
                                for i in 0..data_size {
                                    let key = if i % 5 < 4 {
                                        (i % hot_keys) as i32  // 80% access to hot keys
                                    } else {
                                        i as i32  // 20% access to cold keys
                                    };

                                    if cache.get(&key).is_none() {
                                        cache.insert(key, format!("value_{}", key));
                                    }
                                }
                            }
                            _ => unreachable!(),
                        }

                        cache.len()
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmarks complex access patterns that should differentiate prefetch strategies
fn bench_lru_complex_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_complex_patterns");

    let cache_size = 150;
    let data_size = 800;

    // Complex patterns designed to test prefetch effectiveness
    let complex_patterns = vec![
        ("stride_2", "Access every 2nd element - should benefit Sequential"),
        ("stride_3", "Access every 3rd element - tests adaptability"),
        ("alternating", "A-B-A-B pattern - should benefit Markov learning"),
        ("burst_sequential", "Bursts of sequential access with gaps"),
        ("fibonacci_like", "Fibonacci-like pattern - complex for Markov"),
    ];

    for (pattern_name, _description) in complex_patterns {
        for &prefetch_type in PrefetchType::all() {
            let strategy_name = prefetch_type.name();
            
            group.bench_function(
                format!("{}_{}", strategy_name, pattern_name),
                |b| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, prefetch_type);

                        match pattern_name {
                            "stride_2" => {
                                // Access every 2nd element
                                for i in (0..data_size).step_by(2) {
                                    let key = i as i32;
                                    if cache.get(&key).is_none() {
                                        cache.insert(key, format!("value_{}", key));
                                    }
                                }
                            }
                            "stride_3" => {
                                // Access every 3rd element
                                for i in (0..data_size).step_by(3) {
                                    let key = i as i32;
                                    if cache.get(&key).is_none() {
                                        cache.insert(key, format!("value_{}", key));
                                    }
                                }
                            }
                            "alternating" => {
                                // A-B-A-B alternating pattern
                                let mut toggle = false;
                                for i in 0..data_size {
                                    let key = if toggle { 
                                        (i % 10) as i32 
                                    } else { 
                                        (i % 10 + 100) as i32 
                                    };
                                    toggle = !toggle;
                                    
                                    if cache.get(&key).is_none() {
                                        cache.insert(key, format!("value_{}", key));
                                    }
                                }
                            }
                            "burst_sequential" => {
                                // Bursts of 5 sequential accesses, then jump
                                let burst_size = 5;
                                let mut base = 0i32;
                                
                                for _burst in 0..(data_size / burst_size) {
                                    for i in 0..burst_size {
                                        let key = base + i as i32;
                                        if cache.get(&key).is_none() {
                                            cache.insert(key, format!("value_{}", key));
                                        }
                                    }
                                    base += (burst_size * 3) as i32; // Jump ahead
                                }
                            }
                            "fibonacci_like" => {
                                // Fibonacci-like access pattern
                                let mut a = 1i32;
                                let mut b = 1i32;
                                
                                for _ in 0..(data_size / 10) {
                                    let key_a = a % (data_size as i32);
                                    let key_b = b % (data_size as i32);
                                    
                                    if cache.get(&key_a).is_none() {
                                        cache.insert(key_a, format!("value_{}", key_a));
                                    }
                                    if cache.get(&key_b).is_none() {
                                        cache.insert(key_b, format!("value_{}", key_b));
                                    }
                                    
                                    let next = a.saturating_add(b); // Prevent overflow
                                    a = b;
                                    b = next;
                                }
                            }
                            _ => unreachable!(),
                        }

                        cache.len()
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmarks cache capacity scaling across prefetch strategies
fn bench_lru_capacity_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_capacity_scaling");
    group.sample_size(20); // Reduce samples for large benchmarks

    let capacities = vec![100, 500, 1000, 2000];

    for &capacity in &capacities {
        for &prefetch_type in PrefetchType::all() {
            let strategy_name = prefetch_type.name();
            
            group.bench_with_input(
                BenchmarkId::new(
                    format!("{}_fill_and_evict", strategy_name),
                    capacity
                ),
                &(capacity, prefetch_type),
                |b, &(capacity, prefetch_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(capacity, prefetch_type);

                        // Fill to capacity with sequential pattern
                        for i in 0..capacity {
                            cache.insert(i as i32, format!("initial_{}", i));
                        }

                        // Cause evictions with continued sequential access
                        for i in capacity..(capacity * 2) {
                            cache.insert(i as i32, format!("evict_{}", i));
                        }

                        cache.len()
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmarks prefetch effectiveness by measuring statistics
fn bench_prefetch_effectiveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_effectiveness");

    let cache_size = 100;
    let access_rounds = 3; // Multiple rounds to see prefetch learning

    // Patterns designed to test prefetch learning and effectiveness
    let test_patterns = vec![
        ("repeated_sequential", "Same sequential pattern repeated"),
        ("predictable_cycle", "Predictable cycling pattern"),
        ("mixed_sequential", "Mixed sequential with some randomness"),
    ];

    for (pattern_name, _description) in test_patterns {
        for &prefetch_type in PrefetchType::all() {
            let strategy_name = prefetch_type.name();
            
            group.bench_function(
                format!("{}_{}", strategy_name, pattern_name),
                |b| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, prefetch_type);

                        for round in 0..access_rounds {
                            match pattern_name {
                                "repeated_sequential" => {
                                    // Same sequential pattern each round
                                    for i in 0..50 {
                                        let key = (i + (round * 1000)) as i32; // Offset to avoid cache hits between rounds
                                        if cache.get(&key).is_none() {
                                            cache.insert(key, format!("value_{}", key));
                                        }
                                    }
                                }
                                "predictable_cycle" => {
                                    // A-B-C-D-A-B-C-D pattern
                                    let cycle = vec![1i32, 5, 10, 15];
                                    for &offset in &cycle {
                                        for i in 0..10 {
                                            let key = offset + i as i32 + (round * 1000) as i32;
                                            if cache.get(&key).is_none() {
                                                cache.insert(key, format!("value_{}", key));
                                            }
                                        }
                                    }
                                }
                                "mixed_sequential" => {
                                    // Mostly sequential with some random access
                                    for i in 0..40 {
                                        let key = if i % 7 == 0 {
                                            ((i * 13) % 200 + (round * 1000)) as i32 // Some randomness
                                        } else {
                                            (i + (round * 1000)) as i32 // Mostly sequential
                                        };
                                        
                                        if cache.get(&key).is_none() {
                                            cache.insert(key, format!("value_{}", key));
                                        }
                                    }
                                }
                                _ => unreachable!(),
                            }
                        }

                        // Return both cache length and prefetch stats for analysis
                        let stats = cache.prefetch_stats();
                        (cache.len(), stats.predictions_made, stats.prefetch_hits)
                    })
                },
            );
        }
    }

    group.finish();
}

/// Stress test with high contention and rapid evictions
fn bench_lru_stress_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_stress_test");
    group.sample_size(10); // Fewer samples for stress tests

    let small_cache = 50; // Small cache to force frequent evictions
    let large_dataset = 2000; // Much larger than cache

    for &prefetch_type in PrefetchType::all() {
        let strategy_name = prefetch_type.name();
        
        group.bench_function(
            format!("{}_high_eviction", strategy_name),
            |b| {
                b.iter(|| {
                    let mut cache = create_lru_cache_with_prefetch(small_cache, prefetch_type);

                    // Access pattern that will cause constant evictions
                    for i in 0..large_dataset {
                        let key = match i % 4 {
                            0 => i as i32,                    // New keys
                            1 => (i / 2) as i32,             // Some backward references
                            2 => ((i * 3) % 100) as i32,     // Some cycling
                            _ => (i + 1) as i32,             // Slight forward jump
                        };
                        
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("stress_value_{}", key));
                        }
                    }

                    let stats = cache.prefetch_stats();
                    (cache.len(), stats.hit_rate())
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_lru_basic_operations,
    bench_lru_access_patterns,
    bench_lru_complex_patterns,
    bench_lru_capacity_scaling,
    bench_prefetch_effectiveness,
    bench_lru_stress_test
);

criterion_main!(benches);
