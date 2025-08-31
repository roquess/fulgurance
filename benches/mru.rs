use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::policies::MruCache;

/// Benchmarks basic MRU cache operations
fn bench_mru_basic_operations(c: &mut Criterion) {
    println!("Starting bench_mru_basic_operations...");
    let mut group = c.benchmark_group("mru_basic");

    let cache_sizes = vec![100, 500, 1000];
    let operation_counts = vec![1000, 5000];

    for &cache_size in &cache_sizes {
        for &op_count in &operation_counts {
            group.bench_with_input(
                BenchmarkId::new("insert_get", format!("cap{}_ops{}", cache_size, op_count)),
                &(cache_size, op_count),
                |b, &(cache_size, op_count)| {
                    b.iter(|| {
                        let mut cache = MruCache::new(cache_size);

                        // Insert operations
                        for i in 0..op_count {
                            cache.insert(i, format!("value_{}", i));
                        }

                        // Get operations
                        for i in 0..op_count {
                            let _ = cache.get(&i);
                        }

                        cache.len()
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmarks MRU cache with different access patterns
fn bench_mru_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("mru_patterns");

    let cache_size = 200;
    let data_size = 1000;

    // Sequential access pattern - MRU should perform differently than LRU
    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut cache = MruCache::new(cache_size);

            for i in 0..data_size {
                if cache.get(&i).is_none() {
                    cache.insert(i, format!("value_{}", i));
                }
            }

            cache.len()
        })
    });

    // Random access pattern (simplified)
    group.bench_function("random", |b| {
        b.iter(|| {
            let mut cache = MruCache::new(cache_size);

            for i in 0..data_size {
                let key = (i * 17) % (data_size * 2); // Simple pseudo-random
                if cache.get(&key).is_none() {
                    cache.insert(key, format!("value_{}", key));
                }
            }

            cache.len()
        })
    });

    // Working set pattern (80/20 rule) - MRU should behave inversely to LRU
    group.bench_function("working_set", |b| {
        let hot_keys = data_size / 5; // 20% of keys are "hot"

        b.iter(|| {
            let mut cache = MruCache::new(cache_size);

            for i in 0..data_size {
                let key = if i % 5 < 4 {
                    i % hot_keys  // 80% access to hot keys
                } else {
                    i  // 20% access to cold keys
                };

                if cache.get(&key).is_none() {
                    cache.insert(key, format!("value_{}", key));
                }
            }

            cache.len()
        })
    });

    // Scan pattern - where MRU typically excels
    group.bench_function("scan_pattern", |b| {
        b.iter(|| {
            let mut cache = MruCache::new(cache_size);

            // Simulate large sequential scans where recently accessed data
            // is unlikely to be accessed again soon
            for scan in 0..5 {
                let start = scan * (data_size / 5);
                let end = start + (data_size / 5);
                
                for i in start..end {
                    if cache.get(&i).is_none() {
                        cache.insert(i, format!("scan_{}_{}", scan, i));
                    }
                }
            }

            cache.len()
        })
    });

    group.finish();
}

/// Benchmarks LRU vs MRU direct comparison
fn bench_lru_vs_mru_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_vs_mru");

    let cache_size = 100;
    let data_size = 500;

    // Sequential access - should favor LRU over MRU
    group.bench_function("sequential_lru", |b| {
        b.iter(|| {
            let mut cache = LruCache::new(cache_size);

            for i in 0..data_size {
                if cache.get(&i).is_none() {
                    cache.insert(i, format!("data_{}", i));
                }
            }

            cache.len()
        })
    });

    group.bench_function("sequential_mru", |b| {
        b.iter(|| {
            let mut cache = MruCache::new(cache_size);

            for i in 0..data_size {
                if cache.get(&i).is_none() {
                    cache.insert(i, format!("data_{}", i));
                }
            }

            cache.len()
        })
    });

    // Anti-temporal locality - should favor MRU
    group.bench_function("anti_temporal_lru", |b| {
        b.iter(|| {
            let mut cache = LruCache::new(cache_size);

            // Access pattern where recently used items are less likely to be reused
            for cycle in 0..10 {
                for i in (0..cache_size).rev() {
                    let key = cycle * cache_size + i;
                    if cache.get(&key).is_none() {
                        cache.insert(key, format!("anti_{}", key));
                    }
                }
            }

            cache.len()
        })
    });

    group.bench_function("anti_temporal_mru", |b| {
        b.iter(|| {
            let mut cache = MruCache::new(cache_size);

            // Same anti-temporal pattern
            for cycle in 0..10 {
                for i in (0..cache_size).rev() {
                    let key = cycle * cache_size + i;
                    if cache.get(&key).is_none() {
                        cache.insert(key, format!("anti_{}", key));
                    }
                }
            }

            cache.len()
        })
    });

    // Loop reference pattern - classic MRU use case
    group.bench_function("loop_reference_lru", |b| {
        b.iter(|| {
            let mut cache = LruCache::new(cache_size);

            // Simulate nested loop where outer loop variables
            // are accessed less frequently but need to stay in cache
            for outer in 0..20 {
                // Access outer loop data
                if cache.get(&(outer * 1000)).is_none() {
                    cache.insert(outer * 1000, format!("outer_{}", outer));
                }
                
                // Inner loop creates many accesses that should be evicted
                for inner in 0..50 {
                    let inner_key = outer * 1000 + inner + 1;
                    if cache.get(&inner_key).is_none() {
                        cache.insert(inner_key, format!("inner_{}_{}", outer, inner));
                    }
                }
            }

            cache.len()
        })
    });

    group.bench_function("loop_reference_mru", |b| {
        b.iter(|| {
            let mut cache = MruCache::new(cache_size);

            // Same loop pattern - MRU should perform better
            for outer in 0..20 {
                if cache.get(&(outer * 1000)).is_none() {
                    cache.insert(outer * 1000, format!("outer_{}", outer));
                }
                
                for inner in 0..50 {
                    let inner_key = outer * 1000 + inner + 1;
                    if cache.get(&inner_key).is_none() {
                        cache.insert(inner_key, format!("inner_{}_{}", outer, inner));
                    }
                }
            }

            cache.len()
        })
    });

    group.finish();
}

/// Benchmarks MRU capacity scaling
fn bench_mru_capacity_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("mru_scaling");
    group.sample_size(20); // Reduce samples for large benchmarks

    let capacities = vec![100, 500, 1000, 2000, 5000];

    for &capacity in &capacities {
        group.bench_with_input(
            BenchmarkId::new("fill_and_evict", capacity),
            &capacity,
            |b, &capacity| {
                b.iter(|| {
                    let mut cache = MruCache::new(capacity);

                    // Fill to capacity
                    for i in 0..capacity {
                        cache.insert(i, format!("initial_{}", i));
                    }

                    // Cause evictions - MRU will evict most recently used
                    for i in capacity..(capacity * 2) {
                        cache.insert(i, format!("evict_{}", i));
                    }

                    cache.len()
                })
            },
        );
    }

    group.finish();
}

/// Benchmarks mixed workloads with both policies
fn bench_mixed_workloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workloads");
    
    let cache_size = 200;

    // Database-like workload with index scans
    group.bench_function("db_scan_lru", |b| {
        b.iter(|| {
            let mut cache = LruCache::new(cache_size);
            
            // Simulate table scan followed by index lookups
            for scan_pass in 0..3 {
                // Table scan phase
                for i in 0..300 {
                    let key = scan_pass * 1000 + i;
                    if cache.get(&key).is_none() {
                        cache.insert(key, format!("scan_{}_{}", scan_pass, i));
                    }
                }
                
                // Index lookup phase - reuse some older data
                for i in (0..50).step_by(5) {
                    let key = i;
                    if cache.get(&key).is_none() {
                        cache.insert(key, format!("index_{}", i));
                    }
                }
            }

            cache.len()
        })
    });

    group.bench_function("db_scan_mru", |b| {
        b.iter(|| {
            let mut cache = MruCache::new(cache_size);
            
            // Same database workload
            for scan_pass in 0..3 {
                for i in 0..300 {
                    let key = scan_pass * 1000 + i;
                    if cache.get(&key).is_none() {
                        cache.insert(key, format!("scan_{}_{}", scan_pass, i));
                    }
                }
                
                for i in (0..50).step_by(5) {
                    let key = i;
                    if cache.get(&key).is_none() {
                        cache.insert(key, format!("index_{}", i));
                    }
                }
            }

            cache.len()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_mru_basic_operations,
    bench_mru_access_patterns,
    bench_lru_vs_mru_comparison,
    bench_mru_capacity_scaling,
    bench_mixed_workloads
);

criterion_main!(benches);
