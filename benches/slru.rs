use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::policies::{SlruCache};
use fulgurance::prefetch::PrefetchType;

/// Returns all available prefetch strategies for comparison.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create an SLRU cache with the specified prefetch strategy.
fn create_slru_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> SlruCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => SlruCache::new(capacity),
        _ => SlruCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Bench: Insert + Get pattern - tests basic cache operations
fn bench_insert_then_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("SLRU Insert+Get Pattern");
    let sizes = vec![100, 500, 1000, 2000];
    for &size in &sizes {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), size),
                &(size, pf_type),
                |b, &(size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(size / 2, pf_type);
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
    let mut group = c.benchmark_group("SLRU Sequential Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(cache_size, pf_type);
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
    let mut group = c.benchmark_group("SLRU Random Pattern");
    let configs = vec![(100, 500), (200, 1000), (500, 2000)];
    for &(cache_size, data_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", cache_size, data_size)),
                &(cache_size, data_size, pf_type),
                |b, &(cache_size, data_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: Segment promotion effectiveness - tests SLRU's probationary -> protected promotion
fn bench_segment_promotion(c: &mut Criterion) {
    let mut group = c.benchmark_group("SLRU Segment Promotion");
    let configs = vec![(100, 300), (200, 600), (150, 450)];
    for &(cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_iter{}", cache_size, iterations)),
                &(cache_size, iterations, pf_type),
                |b, &(cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(cache_size, pf_type);
                        
                        // Phase 1: Fill probationary segment
                        for i in 0..cache_size / 2 {
                            cache.insert(i as i32, format!("prob_{i}"));
                        }
                        
                        // Phase 2: Access some items twice to promote them
                        for i in 0..cache_size / 4 {
                            let key = i as i32;
                            let _ = cache.get(&key); // First access (stays in probationary)
                            let _ = cache.get(&key); // Second access (promotes to protected)
                        }
                        
                        // Phase 3: Add more entries to test eviction behavior
                        for i in (cache_size / 2)..iterations {
                            cache.insert(i as i32, format!("new_{i}"));
                        }
                        
                        cache.len()
                    })
                },
            );
        }
    }
    group.finish();
}

/// Bench: Scan resistance - SLRU should protect frequent items during scans
fn bench_scan_resistance(c: &mut Criterion) {
    let mut group = c.benchmark_group("SLRU Scan Resistance");
    let configs = vec![(100, 200, 300), (150, 300, 450), (200, 400, 600)];
    for &(cache_size, working_set, scan_size) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_ws{}_scan{}", cache_size, working_set, scan_size)),
                &(cache_size, working_set, scan_size, pf_type),
                |b, &(cache_size, working_set, scan_size, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(cache_size, pf_type);
                        
                        // Phase 1: Establish working set and promote to protected
                        for _ in 0..3 {
                            for i in 0..working_set {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("ws_{key}"));
                                }
                            }
                        }
                        
                        // Phase 2: Large scan that should not evict protected items
                        for i in working_set..(working_set + scan_size) {
                            cache.insert(i as i32, format!("scan_{i}"));
                        }
                        
                        // Phase 3: Verify working set items still accessible
                        let mut survivors = 0;
                        for i in 0..working_set {
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

/// Bench: Working set pattern (80/20 rule) - where SLRU should excel
fn bench_working_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("SLRU Working Set Pattern (80/20)");
    let configs = vec![(100, 500), (200, 1000), (300, 1500)];
    for &(cache_size, total_accesses) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_acc{}", cache_size, total_accesses)),
                &(cache_size, total_accesses, pf_type),
                |b, &(cache_size, total_accesses, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(cache_size, pf_type);
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

/// Bench: One-time access protection - tests SLRU's ability to quickly evict one-time accesses
fn bench_one_time_access_protection(c: &mut Criterion) {
    let mut group = c.benchmark_group("SLRU One-time Access Protection");
    let configs = vec![(100, 50, 200), (150, 75, 300), (200, 100, 400)];
    for &(cache_size, frequent_keys, one_time_keys) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_freq{}_onetime{}", cache_size, frequent_keys, one_time_keys)),
                &(cache_size, frequent_keys, one_time_keys, pf_type),
                |b, &(cache_size, frequent_keys, one_time_keys, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(cache_size, pf_type);
                        
                        // Phase 1: Establish frequently accessed items
                        for _ in 0..5 {
                            for i in 0..frequent_keys {
                                let key = i as i32;
                                if cache.get(&key).is_none() {
                                    cache.insert(key, format!("freq_{key}"));
                                }
                            }
                        }
                        
                        // Phase 2: Add many one-time access items
                        for i in frequent_keys..(frequent_keys + one_time_keys) {
                            cache.insert(i as i32, format!("onetime_{i}"));
                        }
                        
                        // Phase 3: Check if frequent items survived
                        let mut survivors = 0;
                        for i in 0..frequent_keys {
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

/// Bench: Segment balance effectiveness - tests the 80/20 split behavior
fn bench_segment_balance(c: &mut Criterion) {
    let mut group = c.benchmark_group("SLRU Segment Balance (80/20 split)");
    let configs = vec![(100, 400), (200, 800), (150, 600)];
    for &(cache_size, iterations) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_iter{}", cache_size, iterations)),
                &(cache_size, iterations, pf_type),
                |b, &(cache_size, iterations, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(cache_size, pf_type);
                        
                        // Create diverse access patterns to test segment balance
                        for i in 0..iterations {
                            let key = match i % 10 {
                                // 30% - very frequent (should end up in protected)
                                0..=2 => (i % 10) as i32,
                                // 20% - moderately frequent (should promote to protected) 
                                3..=4 => ((i / 2) % 20 + 100) as i32,
                                // 50% - infrequent/one-time (should stay in probationary)
                                _ => (i + 1000) as i32,
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

/// Bench: High eviction stress test
fn bench_high_eviction_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("SLRU High Eviction Stress Test");
    group.sample_size(10);
    let configs = vec![(50, 1000), (100, 2000), (75, 1500)];
    for &(small_cache, large_dataset) in &configs {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("cache{}_data{}", small_cache, large_dataset)),
                &(small_cache, large_dataset, pf_type),
                |b, &(small_cache, large_dataset, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_slru_cache_with_prefetch(small_cache, pf_type);
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
    bench_segment_promotion,
    bench_scan_resistance,
    bench_working_set,
    bench_one_time_access_protection,
    bench_segment_balance,
    bench_high_eviction_stress
);
criterion_main!(benches);
