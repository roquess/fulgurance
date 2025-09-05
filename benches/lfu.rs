use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::prefetch::PrefetchType;
use fulgurance::policies::LfuCache;

/// Returns all available prefetch strategies for comparison.
/// This automatically adapts whenever you add new PrefetchType variants.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create an LFU cache with the specified prefetch strategy.
fn create_lfu_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> LfuCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => LfuCache::new(capacity),
        _ => LfuCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Benchmarks basic LFU cache operations with prefetch support
fn bench_lfu_basic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("LFU Basic Operations");
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
                            let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

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

/// Benchmarks LFU cache with different access patterns and prefetch support
fn bench_lfu_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("LFU Access Patterns");
    let cache_size = 200;
    let data_size = 1000;

    // Sequential access pattern
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "sequential"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

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
                    let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

                    for i in 0..data_size {
                        let key = ((i * 17) % (data_size * 2)) as i32;
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("value_{}", key));
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Working set pattern (80/20 rule) - should work well with LFU
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "working_set"),
            &pf_type,
            |b, &pf_type| {
                let hot_keys = data_size / 5; // 20% of keys are "hot"
                
                b.iter(|| {
                    let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

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

    group.finish();
}

/// Benchmarks LFU frequency-based eviction patterns
fn bench_lfu_frequency_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("LFU Frequency Patterns");
    let cache_size = 150;

    // Zipf-like distribution - few keys accessed very frequently
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "zipf_distribution"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

                    for round in 0..10 {
                        // Very frequent keys (accessed 10 times per round)
                        for _ in 0..10 {
                            for i in 0..10 {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("hot_{}", key));
                                }
                            }
                        }

                        // Moderately frequent keys (accessed 3 times per round)
                        for _ in 0..3 {
                            for i in 10..30 {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("warm_{}", key));
                                }
                            }
                        }

                        // Infrequent keys (accessed once per round)
                        for i in 30..(30 + round * 20) {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("cold_{}", key));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Age-out pattern - older frequently used items should survive
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "age_out_pattern"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

                    // Phase 1: Build up frequency for some keys
                    for _ in 0..5 {
                        for i in 0..20 {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("frequent_{}", key));
                            }
                        }
                    }

                    // Phase 2: Introduce many new keys (should not evict frequent ones)
                    for i in 100..400 {
                        let key = i as i32;
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("new_{}", key));
                        }
                    }

                    // Phase 3: Access frequent keys again (should still be there)
                    for i in 0..20 {
                        let key = i as i32;
                        cache.get(&key);
                    }

                    cache.len()
                })
            },
        );
    }

    group.finish();
}

/// Benchmarks LFU vs other policies on frequency-sensitive workloads
fn bench_lfu_vs_others_frequency(c: &mut Criterion) {
    let mut group = c.benchmark_group("LFU vs Others - Frequency Workloads");
    let cache_size = 100;

    // Repeated access pattern - LFU should excel here
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "repeated_access_lfu"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

                    // Access pattern: some keys accessed multiple times
                    for cycle in 0..10 {
                        // High frequency keys (accessed every cycle)
                        for i in 0..10 {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("high_freq_{}", key));
                            }
                        }

                        // Medium frequency keys (accessed every 2 cycles)
                        if cycle % 2 == 0 {
                            for i in 10..30 {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("med_freq_{}", key));
                                }
                            }
                        }

                        // Low frequency keys (accessed every 5 cycles)
                        if cycle % 5 == 0 {
                            for i in 30..50 {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("low_freq_{}", key));
                                }
                            }
                        }

                        // One-time keys
                        for i in (cycle * 50)..((cycle + 1) * 50) {
                            let key = (100 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("once_{}", key));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "repeated_access_lru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = match pf_type {
                        PrefetchType::None => LruCache::new(cache_size),
                        _ => LruCache::with_prefetch_i32(cache_size, pf_type),
                    };

                    // Same access pattern for comparison
                    for cycle in 0..10 {
                        for i in 0..10 {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("high_freq_{}", key));
                            }
                        }

                        if cycle % 2 == 0 {
                            for i in 10..30 {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("med_freq_{}", key));
                                }
                            }
                        }

                        if cycle % 5 == 0 {
                            for i in 30..50 {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("low_freq_{}", key));
                                }
                            }
                        }

                        for i in (cycle * 50)..((cycle + 1) * 50) {
                            let key = (100 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("once_{}", key));
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

/// Benchmarks LFU capacity scaling with prefetch
fn bench_lfu_capacity_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("LFU Capacity Scaling");
    group.sample_size(20); // Reduce samples for large benchmarks

    let capacities = vec![100, 500, 1000, 2000, 5000];

    for &capacity in &capacities {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("frequency_build_{}", capacity)),
                &(capacity, pf_type),
                |b, &(capacity, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lfu_cache_with_prefetch(capacity, pf_type);

                        // Build frequency counts
                        for round in 0..5 {
                            for i in 0..capacity {
                                let key = i as i32;
                                // Access frequency decreases with key value
                                let access_count = capacity / (i + 1);
                                for _ in 0..access_count.min(10) {
                                    if cache.get(&key).is_none() {
                                        cache.insert(key, format!("freq_{}_{}", round, key));
                                    }
                                }
                            }
                        }

                        // Cause evictions with new keys
                        for i in capacity..(capacity * 2) {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("evict_{}", key));
                            }
                        }

                        cache.len()
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmarks LFU with temporal locality variations
fn bench_lfu_temporal_variations(c: &mut Criterion) {
    let mut group = c.benchmark_group("LFU Temporal Variations");
    let cache_size = 200;

    // Burst access pattern - frequent bursts followed by quiet periods
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "burst_pattern"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

                    for burst in 0..10 {
                        // Intense burst of accesses
                        for _ in 0..20 {
                            for i in 0..10 {
                                let key = (burst * 10 + i) as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("burst_{}_{}", burst, key));
                                }
                            }
                        }

                        // Quiet period with different keys
                        for i in 0..50 {
                            let key = (1000 + burst * 50 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("quiet_{}_{}", burst, key));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Seasonal pattern - keys popular in certain "seasons"
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "seasonal_pattern"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_lfu_cache_with_prefetch(cache_size, pf_type);

                    for season in 0..4 {
                        // Each season has its own popular keys
                        let season_base = season * 100;
                        
                        // Multiple rounds in each season
                        for _ in 0..5 {
                            // Season-specific popular keys
                            for i in 0..20 {
                                let key = (season_base + i) as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("season_{}_{}", season, key));
                                }
                            }
                            
                            // Some keys are popular across seasons
                            for i in 0..5 {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("global_{}", key));
                                }
                            }
                            
                            // Random background access
                            for i in 0..10 {
                                let key = (500 + season * 20 + i) as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("bg_{}_{}", season, key));
                                }
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
    bench_lfu_basic_operations,
    bench_lfu_access_patterns,
    bench_lfu_frequency_patterns,
    bench_lfu_vs_others_frequency,
    bench_lfu_capacity_scaling,
    bench_lfu_temporal_variations
);

criterion_main!(benches);
