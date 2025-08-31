use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use fulgurance::policies::LfuCache;
use fulgurance::CachePolicy; 

fn bench_lfu_basic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("lfu_basic");
    let cache_sizes = vec![100, 500, 1000];
    let operation_counts = vec![1000, 5000];
    for &cache_size in &cache_sizes {
        for &op_count in &operation_counts {
            group.bench_with_input(
                BenchmarkId::new("insert_get", format!("cap{}_ops{}", cache_size, op_count)),
                &(cache_size, op_count),
                |b, &(cache_size, op_count)| {
                    b.iter(|| {
                        let mut cache = LfuCache::new(cache_size);
                        for i in 0..op_count {
                            cache.insert(i, format!("value_{}", i));
                        }
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

fn bench_lfu_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("lfu_patterns");
    let cache_size = 200;
    let data_size = 1000;

    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut cache = LfuCache::new(cache_size);
            for i in 0..data_size {
                if cache.get(&i).is_none() {
                    cache.insert(i, format!("value_{}", i));
                }
            }
            cache.len()
        })
    });

    group.bench_function("random", |b| {
        b.iter(|| {
            let mut cache = LfuCache::new(cache_size);
            for i in 0..data_size {
                let key = (i * 17) % (data_size * 2);
                if cache.get(&key).is_none() {
                    cache.insert(key, format!("value_{}", key));
                }
            }
            cache.len()
        })
    });

    group.bench_function("working_set", |b| {
        let hot_keys = data_size / 5;
        b.iter(|| {
            let mut cache = LfuCache::new(cache_size);
            for i in 0..data_size {
                let key = if i % 5 < 4 { i % hot_keys } else { i };
                if cache.get(&key).is_none() {
                    cache.insert(key, format!("value_{}", key));
                }
            }
            cache.len()
        })
    });

    group.finish();
}

criterion_group!(benches, bench_lfu_basic_operations, bench_lfu_access_patterns);
criterion_main!(benches);

