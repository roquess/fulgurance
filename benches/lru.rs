use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::prefetch::PrefetchType;

/// Returns all available prefetch strategies for comparison.
/// This automatically adapts whenever you add new PrefetchType variants.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create a cache with the specified prefetch strategy.
fn create_lru_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> LruCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => LruCache::new(capacity),
        _ => LruCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Bench: Insert + Get pattern
fn bench_insert_then_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("Insert+Get Pattern");
    let sizes = vec![100, 500, 1000, 2000];
    for &size in &sizes {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), size),
                &(size, pf_type),
                |b, &(size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(size / 2, pf_type);
                        for i in 0..size {
                            cache.insert(i as i32, format!("value_{i}"));
                        }
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
    let mut group = c.benchmark_group("Sequential Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, pf_type);
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
    let mut group = c.benchmark_group("Random Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Stride access pattern (every Nth element)
fn bench_stride(c: &mut Criterion) {
    let mut group = c.benchmark_group("Stride Pattern");
    let configs = vec![(2, 100, 400), (3, 150, 600), (5, 200, 1000)];
    for &(stride, cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("stride{}_cache{}", stride, cache_size)),
                &(stride, cache_size, data_size, pf_type),
                |b, &(stride, cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, pf_type);
                        for i in (0..data_size).step_by(stride) {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("stride_{key}"));
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

/// Bench: Cyclic pattern (A-B-C-A-B-C...)
fn bench_cyclic(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cyclic Pattern");
    let configs = vec![(3, 100, 300), (5, 150, 500), (7, 200, 700)];
    for &(cycle_length, cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cycle{}_cache{}", cycle_length, cache_size)),
                &(cycle_length, cache_size, iterations, pf_type),
                |b, &(cycle_length, cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, pf_type);
                        for i in 0..iterations {
                            let key = (i % cycle_length + i / cycle_length * 10) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("cycle_{key}"));
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

/// Bench: Working set pattern (80/20 rule)
fn bench_working_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("Working Set Pattern (80/20)");
    let configs = vec![(100, 500), (200, 1000), (300, 1500)];
    for &(cache_size, total_accesses) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_acc{}", cache_size, total_accesses)),
                &(cache_size, total_accesses, pf_type),
                |b, &(cache_size, total_accesses, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Burst sequential pattern (bursts of sequential, with jumps).
fn bench_burst_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("Burst Sequential Pattern");
    let configs = vec![(5, 100, 20), (10, 200, 30), (8, 150, 25)];
    for &(burst_size, cache_size, num_bursts) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("burst{}_cache{}", burst_size, cache_size)),
                &(burst_size, cache_size, num_bursts, pf_type),
                |b, &(burst_size, cache_size, num_bursts, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(cache_size, pf_type);
                        let mut base = 0i32;
                        for _ in 0..num_bursts {
                            // Sequential burst
                            for i in 0..burst_size {
                                let key = base + i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("burst_{key}"));
                                }
                            }
                            base += (burst_size * 3) as i32; // Jump ahead
                        }
                        cache.len()
                    })
                },
            );
        }
    }
    group.finish();
}

/// Bench: High eviction stress test (small cache, large dataset)
fn bench_high_eviction_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("High Eviction Stress Test");
    group.sample_size(10); // Fewer samples for stress test
    let configs = vec![(50, 1000), (100, 2000), (75, 1500)];
    for &(small_cache, large_dataset) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", small_cache, large_dataset)),
                &(small_cache, large_dataset, pf_type),
                |b, &(small_cache, large_dataset, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_lru_cache_with_prefetch(small_cache, pf_type);
                        for i in 0..large_dataset {
                            let key = match i % 4 {
                                0 => i as i32,                    // New keys
                                1 => (i / 2) as i32,             // Some backward references
                                2 => ((i * 3) % 100) as i32,     // Some cycling
                                _ => (i + 1) as i32,             // Slight forward jump
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

// Register all benchmarks in the group. Reports will show every PrefetchType on each graph.
criterion_group!(
    benches,
    bench_insert_then_get,
    bench_sequential,
    bench_random,
    bench_stride,
    bench_cyclic,
    bench_working_set,
    bench_burst_sequential,
    bench_high_eviction_stress
);
criterion_main!(benches);

