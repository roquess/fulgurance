use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::prefetch::PrefetchType;
use fulgurance::policies::MruCache;

/// Returns all available prefetch strategies for comparison.
/// This automatically adapts whenever you add new PrefetchType variants.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create an MRU cache with the specified prefetch strategy.
fn create_mru_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> MruCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => MruCache::new(capacity),
        _ => MruCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Benchmarks basic MRU cache operations
fn bench_mru_basic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("MRU Basic Operations");
    let cache_sizes = vec![100, 500, 1000];
    let operation_counts = vec![1000, 5000];

    for &cache_size in &cache_sizes {
        for &op_count in &operation_counts {
            for &pf_type in &all_prefetch_types() {
                group.bench_with_input(
                    BenchmarkId::new(pf_type.name(), format!("cap{}_ops{}", cache_size, op_count)),
                    &(cache_size, op_count, pf_type),
                    |b, &(cache_size, op_count, pf_type)| {
                        b.iter(|| {
                            let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

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

/// Benchmarks MRU cache with different access patterns
fn bench_mru_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("MRU Access Patterns");
    let cache_size = 200;
    let data_size = 1000;

    // Sequential access pattern - MRU should perform differently than LRU
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "sequential"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

                    for i in 0..data_size {
                        let key = i as i32;
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("value_{}", i));
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Random access pattern
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "random"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

                    for i in 0..data_size {
                        let key = ((i * 17) % (data_size * 2)) as i32; // Simple pseudo-random
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("value_{}", key));
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Working set pattern (80/20 rule) - MRU should behave inversely to LRU
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "working_set"),
            &pf_type,
            |b, &pf_type| {
                let hot_keys = data_size / 5; // 20% of keys are "hot"
                
                b.iter(|| {
                    let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

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

                    cache.len()
                })
            },
        );
    }

    // Scan pattern - where MRU typically excels
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "scan_pattern"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

                    // Simulate large sequential scans where recently accessed data
                    // is unlikely to be accessed again soon
                    for scan in 0..5 {
                        let start = scan * (data_size / 5);
                        let end = start + (data_size / 5);

                        for i in start..end {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("scan_{}_{}", scan, i));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    group.finish();
}

/// Benchmarks LRU vs MRU direct comparison with prefetch
fn bench_lru_vs_mru_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("LRU vs MRU Comparison");
    let cache_size = 100;
    let data_size = 500;

    // Sequential access - should favor LRU over MRU
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "sequential_lru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = match pf_type {
                        PrefetchType::None => LruCache::new(cache_size),
                        _ => LruCache::with_prefetch_i32(cache_size, pf_type),
                    };

                    for i in 0..data_size {
                        let key = i as i32;
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("data_{}", i));
                        }
                    }

                    cache.len()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "sequential_mru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

                    for i in 0..data_size {
                        let key = i as i32;
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("data_{}", i));
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Anti-temporal locality - should favor MRU
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "anti_temporal_lru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = match pf_type {
                        PrefetchType::None => LruCache::new(cache_size),
                        _ => LruCache::with_prefetch_i32(cache_size, pf_type),
                    };

                    // Access pattern where recently used items are less likely to be reused
                    for cycle in 0..10 {
                        for i in (0..cache_size).rev() {
                            let key = (cycle * cache_size + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("anti_{}", key));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "anti_temporal_mru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

                    // Same anti-temporal pattern
                    for cycle in 0..10 {
                        for i in (0..cache_size).rev() {
                            let key = (cycle * cache_size + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("anti_{}", key));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Loop reference pattern - classic MRU use case
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "loop_reference_lru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = match pf_type {
                        PrefetchType::None => LruCache::new(cache_size),
                        _ => LruCache::with_prefetch_i32(cache_size, pf_type),
                    };

                    // Simulate nested loop where outer loop variables
                    // are accessed less frequently but need to stay in cache
                    for outer in 0..20 {
                        // Access outer loop data
                        let outer_key = (outer * 1000) as i32;
                        if cache.get(&outer_key).is_none() {
                            cache.insert(outer_key, format!("outer_{}", outer));
                        }

                        // Inner loop creates many accesses that should be evicted
                        for inner in 0..50 {
                            let inner_key = (outer * 1000 + inner + 1) as i32;
                            if cache.get(&inner_key).is_none() {
                                cache.insert(inner_key, format!("inner_{}_{}", outer, inner));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "loop_reference_mru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

                    // Same loop pattern - MRU should perform better
                    for outer in 0..20 {
                        let outer_key = (outer * 1000) as i32;
                        if cache.get(&outer_key).is_none() {
                            cache.insert(outer_key, format!("outer_{}", outer));
                        }

                        for inner in 0..50 {
                            let inner_key = (outer * 1000 + inner + 1) as i32;
                            if cache.get(&inner_key).is_none() {
                                cache.insert(inner_key, format!("inner_{}_{}", outer, inner));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    group.finish();
}

/// Benchmarks MRU capacity scaling with prefetch
fn bench_mru_capacity_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("MRU Capacity Scaling");
    group.sample_size(20); // Reduce samples for large benchmarks

    let capacities = vec![100, 500, 1000, 2000, 5000];

    for &capacity in &capacities {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("fill_and_evict_{}", capacity)),
                &(capacity, pf_type),
                |b, &(capacity, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_mru_cache_with_prefetch(capacity, pf_type);

                        // Fill to capacity
                        for i in 0..capacity {
                            cache.insert(i as i32, format!("initial_{}", i));
                        }

                        // Cause evictions - MRU will evict most recently used
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

/// Benchmarks mixed workloads with both policies and prefetch
fn bench_mixed_workloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("Mixed Workloads");
    let cache_size = 200;

    // Database-like workload with index scans
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "db_scan_lru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = match pf_type {
                        PrefetchType::None => LruCache::new(cache_size),
                        _ => LruCache::with_prefetch_i32(cache_size, pf_type),
                    };

                    // Simulate table scan followed by index lookups
                    for scan_pass in 0..3 {
                        // Table scan phase
                        for i in 0..300 {
                            let key = (scan_pass * 1000 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("scan_{}_{}", scan_pass, i));
                            }
                        }

                        // Index lookup phase - reuse some older data
                        for i in (0..50).step_by(5) {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("index_{}", i));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "db_scan_mru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_mru_cache_with_prefetch(cache_size, pf_type);

                    // Same database workload
                    for scan_pass in 0..3 {
                        for i in 0..300 {
                            let key = (scan_pass * 1000 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("scan_{}_{}", scan_pass, i));
                            }
                        }

                        for i in (0..50).step_by(5) {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("index_{}", i));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );
    }

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
