use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::prelude::*;
use fulgurance::prefetch::PrefetchType;
use fulgurance::policies::FifoCache;

/// Returns all available prefetch strategies for comparison.
/// This automatically adapts whenever you add new PrefetchType variants.
fn all_prefetch_types() -> Vec<PrefetchType> {
    PrefetchType::all().to_vec()
}

/// Helper to create a FIFO cache with the specified prefetch strategy.
fn create_fifo_cache_with_prefetch(capacity: usize, prefetch_type: PrefetchType) -> FifoCache<i32, String> {
    match prefetch_type {
        PrefetchType::None => FifoCache::new(capacity),
        _ => FifoCache::with_prefetch_i32(capacity, prefetch_type),
    }
}

/// Benchmarks basic FIFO cache operations with prefetch support
fn bench_fifo_basic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("FIFO Basic Operations");
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
                            let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

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

/// Benchmarks FIFO cache with different access patterns and prefetch support
fn bench_fifo_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("FIFO Access Patterns");
    let cache_size = 200;
    let data_size = 1000;

    // Sequential access pattern - FIFO works well with streaming data
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "sequential"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

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
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

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

    // Streaming pattern - FIFO's ideal use case
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "streaming"),
            &pf_type,

            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

                    // Simulate continuous data stream processing
                    for batch in 0..10 {
                        let batch_start = batch * 100;
                        for i in 0..200 {  // Process more data than cache can hold
                            let key = (batch_start + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("stream_{}_{}", batch, key));
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

/// Benchmarks FIFO vs other policies on streaming workloads
fn bench_fifo_vs_others_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("FIFO vs Others - Streaming Workloads");
    let cache_size = 150;

    // Pure streaming workload - FIFO should excel
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "pure_stream_fifo"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

                    // Continuous stream of new data
                    for i in 0..1500 {  // 10x cache size
                        let key = i as i32;
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("stream_{}", key));
                        }
                    }

                    cache.len()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "pure_stream_lru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = match pf_type {
                        PrefetchType::None => LruCache::new(cache_size),
                        _ => LruCache::with_prefetch_i32(cache_size, pf_type),
                    };

                    // Same streaming workload
                    for i in 0..1500 {
                        let key = i as i32;
                        if cache.get(&key).is_none() {
                            cache.insert(key, format!("stream_{}", key));
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Mixed streaming with some reuse - LRU should perform better
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "mixed_stream_fifo"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

                    for batch in 0..10 {
                        // Mostly new streaming data
                        for i in 0..100 {
                            let key = (batch * 100 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("new_{}_{}", batch, key));
                            }
                        }

                        // Some reuse of recent data
                        if batch > 0 {
                            for i in 0..10 {
                                let key = ((batch - 1) * 100 + i) as i32;
                                cache.get(&key);
                            }
                        }
                    }

                    cache.len()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "mixed_stream_lru"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = match pf_type {
                        PrefetchType::None => LruCache::new(cache_size),
                        _ => LruCache::with_prefetch_i32(cache_size, pf_type),
                    };

                    for batch in 0..10 {
                        // Mostly new streaming data
                        for i in 0..100 {
                            let key = (batch * 100 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("new_{}_{}", batch, key));
                            }
                        }

                        // Some reuse of recent data
                        if batch > 0 {
                            for i in 0..10 {
                                let key = ((batch - 1) * 100 + i) as i32;
                                cache.get(&key);
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

/// Benchmarks FIFO insertion order behavior
fn bench_fifo_insertion_order(c: &mut Criterion) {
    let mut group = c.benchmark_group("FIFO Insertion Order");
    let cache_size = 100;

    // Test that FIFO maintains strict insertion order
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "strict_order"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

                    // Fill cache completely
                    for i in 0..cache_size {
                        cache.insert(i as i32, format!("initial_{}", i));
                    }

                    // Add more items - should evict in insertion order
                    for i in cache_size..(cache_size * 2) {
                        cache.insert(i as i32, format!("overflow_{}", i));
                    }

                    // Access some older items (shouldn't affect eviction in FIFO)
                    for i in (cache_size / 2)..cache_size {
                        cache.get(&(i as i32));
                    }

                    // Add more items
                    for i in (cache_size * 2)..(cache_size * 3) {
                        cache.insert(i as i32, format!("final_{}", i));
                    }

                    cache.len()
                })
            },
        );
    }

    // Batch processing pattern - good fit for FIFO
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "batch_processing"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

                    for batch in 0..20 {
                        let batch_start = batch * 50;
                        
                        // Process a batch of items
                        for i in 0..80 {  // More items than cache capacity
                            let key = (batch_start + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("batch_{}_{}", batch, i));
                            }
                            
                            // Some processing work simulation
                            cache.get(&key);
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    group.finish();
}

/// Benchmarks FIFO capacity scaling with prefetch
fn bench_fifo_capacity_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("FIFO Capacity Scaling");
    group.sample_size(20); // Reduce samples for large benchmarks

    let capacities = vec![100, 500, 1000, 2000, 5000];

    for &capacity in &capacities {
        for &pf_type in &all_prefetch_types() {
            group.bench_with_input(
                BenchmarkId::new(pf_type.name(), format!("streaming_load_{}", capacity)),
                &(capacity, pf_type),
                |b, &(capacity, pf_type)| {
                    b.iter(|| {
                        let mut cache = create_fifo_cache_with_prefetch(capacity, pf_type);

                        // Simulate heavy streaming load
                        for i in 0..(capacity * 3) {  // 3x cache size data stream
                            let key = i as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("stream_{}", key));
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

/// Benchmarks FIFO pipeline processing patterns
fn bench_fifo_pipeline_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("FIFO Pipeline Patterns");
    let cache_size = 200;

    // Multi-stage pipeline - FIFO as intermediate buffer
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "pipeline_stages"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

                    for pipeline_run in 0..10 {
                        // Stage 1: Input processing
                        for i in 0..150 {
                            let key = (pipeline_run * 1000 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("stage1_{}_{}", pipeline_run, i));
                            }
                        }

                        // Stage 2: Intermediate processing (access recent items)
                        for i in 0..75 {
                            let key = (pipeline_run * 1000 + i) as i32;
                            cache.get(&key);
                        }

                        // Stage 3: Output processing (mostly new data)
                        for i in 200..350 {
                            let key = (pipeline_run * 1000 + i) as i32;
                            if cache.get(&key).is_none() {
                                cache.insert(key, format!("stage3_{}_{}", pipeline_run, i));
                            }
                        }
                    }

                    cache.len()
                })
            },
        );
    }

    // Ring buffer simulation - natural FIFO use case
    for &pf_type in &all_prefetch_types() {
        group.bench_with_input(
            BenchmarkId::new(pf_type.name(), "ring_buffer"),
            &pf_type,
            |b, &pf_type| {
                b.iter(|| {
                    let mut cache = create_fifo_cache_with_prefetch(cache_size, pf_type);

                    // Simulate ring buffer behavior
                    for cycle in 0..50 {
                        // Write new data to buffer
                        for i in 0..20 {
                            let key = (cycle * 20 + i) as i32;
                            cache.insert(key, format!("buffer_{}_{}", cycle, i));
                        }

                        // Read some recent data
                        for i in 0..5 {
                            let key = (cycle * 20 + i) as i32;
                            cache.get(&key);
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
    bench_fifo_basic_operations,
    bench_fifo_access_patterns,
    bench_fifo_vs_others_streaming,
    bench_fifo_insertion_order,
    bench_fifo_capacity_scaling,
    bench_fifo_pipeline_patterns
);

criterion_main!(benches);
