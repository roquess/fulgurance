use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::policies::{ClockCache};
use fulgurance::prefetch::PrefetchType;

/// Returns all available prefetch strategies for comparison.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create a Clock cache with the specified prefetch strategy.
fn create_clock_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> ClockCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => ClockCache::new(capacity),
        _ => ClockCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Bench: Insert + Get pattern - tests basic cache operations
fn bench_insert_then_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("Clock Insert+Get Pattern");
    let sizes = vec![100, 500, 1000, 2000];
    for &size in &sizes {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), size),
                &(size, pf_type),
                |b, &(size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(size / 2, pf_type);
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
    let mut group = c.benchmark_group("Clock Sequential Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(cache_size, pf_type);
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
    let mut group = c.benchmark_group("Clock Random Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Clock hand efficiency - tests Clock's circular replacement mechanism
fn bench_clock_hand_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("Clock Hand Efficiency");
    let configs = vec![(50, 200), (100, 400), (150, 600)];
    for &(cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_iter{}", cache_size, iterations)),
                &(cache_size, iterations, pf_type),
                |b, &(cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(cache_size, pf_type);
                        
                        // Fill cache completely
                        for i in 0..cache_size {
                            cache.insert(i as i32, format!("fill_{i}"));
                        }
                        
                        // Pattern that tests reference bit mechanism
                        for i in 0..iterations {
                            let key = match i % 6 {
                                0 | 1 => (i % cache_size) as i32,    // Recent reaccess (sets ref bit)
                                2 => (i + cache_size) as i32,      // New entry (triggers eviction)
                                3 => (i + cache_size * 2) as i32, // Another new entry
                                4 => (i % (cache_size / 2)) as i32, // Frequent reaccess
                                _ => (i + cache_size * 3) as i32, // More new entries
                            };
                            
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("test_{key}"));
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

/// Bench: Reference bit effectiveness - pattern designed to test second-chance mechanism
fn bench_reference_bit_effectiveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("Clock Reference Bit Effectiveness");
    let configs = vec![(100, 300), (200, 600), (150, 450)];
    for &(cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_iter{}", cache_size, iterations)),
                &(cache_size, iterations, pf_type),
                |b, &(cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(cache_size, pf_type);
                        
                        // Create initial working set
                        for i in 0..cache_size {
                            cache.insert(i as i32, format!("init_{i}"));
                        }
                        
                        // Access pattern that tests reference bits
                        for i in 0..iterations {
                            // Phase 1: Access every other item (sets ref bits on half)
                            if i % 2 == 0 {
                                let key = (i % cache_size) as i32;
                                let _ = cache.get(&key);
                            }
                            
                            // Phase 2: Try to evict with new entries
                            if i % 3 == 0 {
                                let new_key = (i + cache_size * 2) as i32;
                                cache.insert(new_key, format!("new_{new_key}"));
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

/// Bench: Stride access pattern - tests how Clock handles regular patterns
fn bench_stride(c: &mut Criterion) {
    let mut group = c.benchmark_group("Clock Stride Pattern");
    let configs = vec![(2, 100, 400), (3, 150, 600), (5, 200, 1000)];
    for &(stride, cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("stride{}_cache{}", stride, cache_size)),
                &(stride, cache_size, data_size, pf_type),
                |b, &(stride, cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Cyclic pattern - tests Clock's ability to handle repeating access patterns
fn bench_cyclic(c: &mut Criterion) {
    let mut group = c.benchmark_group("Clock Cyclic Pattern");
    let configs = vec![(3, 100, 300), (5, 150, 500), (7, 200, 700)];
    for &(cycle_length, cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cycle{}_cache{}", cycle_length, cache_size)),
                &(cycle_length, cache_size, iterations, pf_type),
                |b, &(cycle_length, cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(cache_size, pf_type);
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
    let mut group = c.benchmark_group("Clock Working Set Pattern (80/20)");
    let configs = vec![(100, 500), (200, 1000), (300, 1500)];
    for &(cache_size, total_accesses) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_acc{}", cache_size, total_accesses)),
                &(cache_size, total_accesses, pf_type),
                |b, &(cache_size, total_accesses, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Approximated LRU effectiveness - how well Clock approximates true LRU
fn bench_lru_approximation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Clock LRU Approximation");
    let configs = vec![(100, 400), (200, 800), (150, 600)];
    for &(cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_iter{}", cache_size, iterations)),
                &(cache_size, iterations, pf_type),
                |b, &(cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(cache_size, pf_type);
                        
                        // Create graduated age pattern
                        for i in 0..cache_size {
                            cache.insert(i as i32, format!("age_{i}"));
                        }
                        
                        // Access older items to make them "younger" via reference bits
                        for i in 0..cache_size / 2 {
                            let _ = cache.get(&(i as i32));
                        }
                        
                        // Insert new items (should evict non-accessed items first)
                        for i in cache_size..iterations {
                            cache.insert(i as i32, format!("new_{i}"));
                        }
                        
                        // Test if recently accessed items survived
                        let mut survivors = 0;
                        for i in 0..cache_size / 2 {
                            if cache.get(&(i as i32)).is_some() {
                                survivors += 1;
                            }
                        }
                        (cache.len(), survivors)
                    })
                },
            );
        }
    }
    group.finish();
}

/// Bench: High eviction stress test
fn bench_high_eviction_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("Clock High Eviction Stress Test");
    group.sample_size(10);
    let configs = vec![(50, 1000), (100, 2000), (75, 1500)];
    for &(small_cache, large_dataset) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", small_cache, large_dataset)),
                &(small_cache, large_dataset, pf_type),
                |b, &(small_cache, large_dataset, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_clock_cache_with_prefetch(small_cache, pf_type);
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
    bench_clock_hand_efficiency,
    bench_reference_bit_effectiveness,
    bench_stride,
    bench_cyclic,
    bench_working_set,
    bench_lru_approximation,
    bench_high_eviction_stress
);
criterion_main!(benches);
