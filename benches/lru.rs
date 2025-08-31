use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;

/// Benchmarks basic LRU cache operations
fn bench_lru_basic_operations(c: &mut Criterion) {
    println!("Starting bench_lru_basic_operations...");
    let mut group = c.benchmark_group("lru_basic");
    
    let cache_sizes = vec![100, 500, 1000];
    let operation_counts = vec![1000, 5000];
    
    for &cache_size in &cache_sizes {
        for &op_count in &operation_counts {
            group.bench_with_input(
                BenchmarkId::new("insert_get", format!("cap{}_ops{}", cache_size, op_count)),
                &(cache_size, op_count),
                |b, &(cache_size, op_count)| {
                    b.iter(|| {
                        let mut cache = LruCache::new(cache_size);
                        
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

/// Benchmarks LRU cache with different access patterns
fn bench_lru_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_patterns");
    
    let cache_size = 200;
    let data_size = 1000;
    
    // Sequential access pattern
    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut cache = LruCache::new(cache_size);
            
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
            let mut cache = LruCache::new(cache_size);
            
            for i in 0..data_size {
                let key = (i * 17) % (data_size * 2); // Simple pseudo-random
                if cache.get(&key).is_none() {
                    cache.insert(key, format!("value_{}", key));
                }
            }
            
            cache.len()
        })
    });
    
    // Working set pattern (80/20 rule)
    group.bench_function("working_set", |b| {
        let hot_keys = data_size / 5; // 20% of keys are "hot"
        
        b.iter(|| {
            let mut cache = LruCache::new(cache_size);
            
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
    
    group.finish();
}

/// Benchmarks Fulgurance integrated cache with prefetching
fn bench_fulgurance_vs_plain_lru(c: &mut Criterion) {
    let mut group = c.benchmark_group("fulgurance_comparison");
    
    let cache_size = 100;
    let data_size = 500;
    
    // Plain LRU cache
    group.bench_function("plain_lru", |b| {
        b.iter(|| {
            let mut cache = LruCache::new(cache_size);
            
            // Sequential access that should benefit from prefetching
            for i in 0..data_size {
                if cache.get(&i).is_none() {
                    cache.insert(i, format!("data_{}", i));
                }
            }
            
            cache.len()
        })
    });
    
    // Fulgurance cache with sequential prefetching
    group.bench_function("fulgurance_sequential", |b| {
        b.iter(|| {
            let cache = LruCache::new(cache_size);
            let prefetch_strategy = SequentialPrefetch::new();
            let mut fulgurance = FulgranceCache::new(cache, prefetch_strategy)
                .with_prefetch_fn(|key: &i32| {
                    Some(format!("data_{}", key))
                });
            
            // Same sequential access pattern
            for i in 0..data_size {
                fulgurance.get(&i);
            }
            
            fulgurance.len()
        })
    });
    
    group.finish();
}

/// Benchmarks cache capacity scaling
fn bench_lru_capacity_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_scaling");
    group.sample_size(20); // Reduce samples for large benchmarks
    
    let capacities = vec![100, 500, 1000, 2000, 5000];
    
    for &capacity in &capacities {
        group.bench_with_input(
            BenchmarkId::new("fill_and_evict", capacity),
            &capacity,
            |b, &capacity| {
                b.iter(|| {
                    let mut cache = LruCache::new(capacity);
                    
                    // Fill to capacity
                    for i in 0..capacity {
                        cache.insert(i, format!("initial_{}", i));
                    }
                    
                    // Cause evictions
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

criterion_group!(
    benches,
    bench_lru_basic_operations,
    bench_lru_access_patterns,
    bench_fulgurance_vs_plain_lru,
    bench_lru_capacity_scaling
);

criterion_main!(benches);
