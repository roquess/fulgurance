use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::policies::TwoQCache;
use fulgurance::prefetch::PrefetchType;

/// Returns all available prefetch strategies for comparison.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create a 2Q cache with the specified prefetch strategy.
fn create_two_q_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> TwoQCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => TwoQCache::new(capacity),
        _ => TwoQCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Bench: Insert + Get pattern
fn bench_insert_then_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("2Q Insert+Get Pattern");
    let sizes = vec![100, 500, 1000, 2000];
    for &size in &sizes {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), size),
                &(size, pf_type),
                |b, &(size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_two_q_cache_with_prefetch(size / 2, pf_type);
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

/// Bench: Sequential access
fn bench_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("2Q Sequential Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_two_q_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Random access
fn bench_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("2Q Random Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_two_q_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Promotion mechanism
fn bench_promotion_mechanism(c: &mut Criterion) {
    let mut group = c.benchmark_group("2Q Promotion Mechanism");
    let configs = vec![(100, 300), (200, 600), (150, 450)];
    for &(cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_iter{}", cache_size, iterations)),
                &(cache_size, iterations, pf_type),
                |b, &(cache_size, _iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_two_q_cache_with_prefetch(cache_size, pf_type);
                        for i in 0..cache_size {
                            cache.insert(i as i32, format!("first_{i}"));
                        }
                        for i in cache_size..(cache_size * 2) {
                            cache.insert(i as i32, format!("evict_{i}"));
                        }
                        for i in 0..cache_size / 2 {
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("promote_{key}"));
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
    let mut group = c.benchmark_group("2Q Working Set Pattern (80/20)");
    let configs = vec![(100, 500), (200, 1000), (300, 1500)];
    for &(cache_size, total_accesses) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_acc{}", cache_size, total_accesses)),
                &(cache_size, total_accesses, pf_type),
                |b, &(cache_size, total_accesses, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_two_q_cache_with_prefetch(cache_size, pf_type);
                        let hot_keys = cache_size / 5;
                        for i in 0..total_accesses {
                            let key = if i % 5 < 4 {
                                (i % hot_keys) as i32
                            } else {
                                i as i32
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

// Register all benchmarks
criterion_group!(
    benches,
    bench_insert_then_get,
    bench_sequential,
    bench_random,
    bench_promotion_mechanism,
    bench_working_set
);
criterion_main!(benches);

