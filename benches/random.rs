use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::prefetch::{PrefetchType, create_prefetch_strategy_i32};
use fulgurance::policies::RandomCache;

/// Returns all available prefetch strategies for comparison.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create a RandomCache with the specified prefetch strategy.
fn create_random_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> RandomCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => RandomCache::new(capacity),
        _ => RandomCache::with_custom_prefetch(
            capacity,
            create_prefetch_strategy_i32(prefetch_type),
        ),
    }
}

/// Bench: Insert + Get pattern for RandomCache
fn bench_random_insert_then_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("RandomCache: Insert+Get Pattern");
    let sizes = vec![100, 500, 1000, 2000];
    for &size in &sizes {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), size),
                &(size, pf_type),
                |b, &(size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_random_cache_with_prefetch(size / 2, pf_type);
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

/// Bench: Sequential access pattern for RandomCache
fn bench_random_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("RandomCache: Sequential Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_random_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Random access pattern for RandomCache
fn bench_random_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("RandomCache: Random Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_random_cache_with_prefetch(cache_size, pf_type);
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

criterion_group!(
    benches,
    bench_random_insert_then_get,
    bench_random_sequential,
    bench_random_random,
);
criterion_main!(benches);

