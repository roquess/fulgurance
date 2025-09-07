use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::policies::CarCache;
use fulgurance::prefetch::PrefetchType;

/// Returns all available prefetch strategies for comparison.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create a CAR cache with the specified prefetch strategy.
fn create_car_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> CarCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => CarCache::new(capacity),
        _ => CarCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Bench: Insert + Get pattern - tests basic cache operations
fn bench_insert_then_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("CAR Insert+Get Pattern");
    let sizes = vec![100, 500, 1000, 2000];
    for &size in &sizes {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), size),
                &(size, pf_type),
                |b, &(size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_car_cache_with_prefetch(size / 2, pf_type);
                        // Insert phase
                        for i in 0..size {
                            cache.insert(i as i32, format!("value_{i}"));
                        }
                        // Get phase
                        for i in 0..size {
                            let _ = cache.get(&(i as i32));
                        }
                        cache.len()
                    })
                },
            );
        }
    }
    group.finish();
}

/// Bench: Sequential access pattern
fn bench_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("CAR Sequential Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_car_cache_with_prefetch(cache_size, pf_type);
                        for i in 0..data_size {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("seq_{key}"));
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

/// Bench: Random access pattern
fn bench_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("CAR Random Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_car_cache_with_prefetch(cache_size, pf_type);
                        for i in 0..data_size {
                            let key = ((i * 17) % (data_size * 2)) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("rand_{key}"));
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

/// Bench: Clock algorithm efficiency - pattern that tests CAR's Clock replacement
fn bench_clock_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("CAR Clock Efficiency");
    let configs = vec![(50, 200), (100, 400), (150, 600)];
    for &(cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_iter{}", cache_size, iterations)),
                &(cache_size, iterations, pf_type),
                |b, &(cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_car_cache_with_prefetch(cache_size, pf_type);
                        
                        // Fill cache to trigger Clock algorithm
                        for i in 0..cache_size * 2 {
                            cache.insert(i as i32, format!("fill_{i}"));
                        }
                        
                        // Mixed access pattern to test reference bits
                        for i in 0..iterations {
                            let key = match i % 4 {
                                0 => (i / 4) as i32,           // Recent access
                                1 => ((i / 4) + 10) as i32,   // Slightly older
                                2 => (i % 20) as i32,         // Frequent reaccess
                                _ => (i + cache_size) as i32, // New entries
                            };
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("mixed_{key}"));
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

/// Bench: Adaptation effectiveness - tests CAR's adaptive behavior
fn bench_adaptation(c: &mut Criterion) {
    let mut group = c.benchmark_group("CAR Adaptation Test");
    let configs = vec![(100, 300), (200, 600), (150, 450)];
    for &(cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_iter{}", cache_size, iterations)),
                &(cache_size, iterations, pf_type),
                |b, &(cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_car_cache_with_prefetch(cache_size, pf_type);
                        
                        // Phase 1: Build working set (should populate T1 then T2)
                        for _ in 0..3 {
                            for i in 0..30 {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("phase1_{key}"));
                                }
                            }
                        }
                        
                        // Phase 2: Mixed pattern to test adaptation
                        for i in 30..iterations {
                            let key = if i % 3 == 0 {
                                // Revisit frequently accessed
                                (i % 30) as i32
                            } else if i % 3 == 1 {
                                // New entries (test B1 ghost hits)
                                (i + 100) as i32
                            } else {
                                // Random within working set
                                ((i * 7) % 50) as i32
                            };
                            
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("phase2_{key}"));
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

/// Bench: Working set pattern (80/20 rule) - where CAR should excel
fn bench_working_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("CAR Working Set Pattern (80/20)");
    let configs = vec![(100, 500), (200, 1000), (300, 1500)];
    for &(cache_size, total_accesses) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_acc{}", cache_size, total_accesses)),
                &(cache_size, total_accesses, pf_type),
                |b, &(cache_size, total_accesses, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_car_cache_with_prefetch(cache_size, pf_type);
                        let hot_keys = cache_size / 5; // 20% "hot" keys
                        for i in 0..total_accesses {
                            let key = if i % 5 < 4 {
                                (i % hot_keys) as i32  // 80% hot keys
                            } else {
                                i as i32               // 20% cold keys
                            };
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("ws_{key}"));
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

/// Bench: Ghost buffer effectiveness - tests B1/B2 ghost lists
fn bench_ghost_effectiveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("CAR Ghost Buffer Effectiveness");
    let configs = vec![(50, 150, 100), (100, 300, 200), (75, 225, 150)];
    for &(cache_size, working_set, revisit_after) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_ws{}", cache_size, working_set)),
                &(cache_size, working_set, revisit_after, pf_type),
                |b, &(cache_size, working_set, revisit_after, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_car_cache_with_prefetch(cache_size, pf_type);
                        
                        // Phase 1: Fill beyond capacity to populate ghost buffers
                        for i in 0..working_set {
                            cache.insert(i as i32, format!("phase1_{i}"));
                        }
                        
                        // Phase 2: Access new items to trigger evictions
                        for i in working_set..(working_set + revisit_after) {
                            cache.insert(i as i32, format!("phase2_{i}"));
                        }
                        
                        // Phase 3: Revisit old items (should benefit from ghost buffers)
                        for i in 0..working_set {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("revisit_{i}"));
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

/// Bench: High eviction stress test
fn bench_high_eviction_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("CAR High Eviction Stress Test");
    group.sample_size(10);
    let configs = vec![(50, 1000), (100, 2000), (75, 1500)];
    for &(small_cache, large_dataset) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", small_cache, large_dataset)),
                &(small_cache, large_dataset, pf_type),
                |b, &(small_cache, large_dataset, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_car_cache_with_prefetch(small_cache, pf_type);
                        for i in 0..large_dataset {
                            let key = match i % 4 {
                                0 => i as i32,
                                1 => (i / 2) as i32,
                                2 => ((i * 3) % 100) as i32,
                                _ => (i + 1) as i32,
                            };
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("stress_{key}"));
                            }
                        }
                        let stats = cache.prefetch_stats();
                        (cache.len(), stats.predictions_made, stats.prefetch_hits)
                    })
                },
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_insert_then_get,
    bench_sequential,
    bench_random,
    bench_clock_efficiency,
    bench_adaptation,
    bench_working_set,
    bench_ghost_effectiveness,
    bench_high_eviction_stress
);
criterion_main!(benches);
