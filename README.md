# Fulgurance

A blazing-fast, adaptive **prefetching and caching library for Rust**.
Fulgurance optimizes memory and disk accesses by predicting and prefetching data before it’s needed, reducing latency and improving performance for **databases, distributed systems, and high-performance applications**.

---

## Features

- **Pluggable cache policies** – Switch between classic and advanced eviction strategies.
- **Flexible prefetching** – Choose from none, sequential, Markov, stride, history-based, or adaptive approaches.
- **Benchmark-ready** – Built with [criterion](https://crates.io/crates/criterion) for performance comparisons.
- **Extensible and efficient** – Zero-cost abstractions, easy to customize and integrate.

---

## Cache Policies

Fulgurance supports a wide range of **cache eviction strategies**, each with a different approach:

- **LRU (Least Recently Used)** – Evicts the item that hasn’t been accessed for the longest time.  
  Assumes recently used items are likely to be used again soon.

- **MRU (Most Recently Used)** – Evicts the most recently accessed item.  
  Useful in workloads where once data is accessed, it is unlikely to be reused immediately.

- **FIFO (First-In, First-Out)** – Evicts items in the order they were inserted.  
  Simple and fair; the oldest data leaves first, regardless of usage.

- **LFU (Least Frequently Used)** – Evicts the item with the lowest access frequency.  
  Keeps the most popular items in the cache.

- **Random** – Evicts a random item when the cache is full.  
  Simple, low overhead, and avoids pathological patterns.

- **ARC (Adaptive Replacement Cache)** – Balances between recency and frequency dynamically.  
  Adapts to changing workloads for better hit rates.

- **Clock** – Approximates LRU using a circular buffer and reference bits.  
  Efficient, low-overhead alternative to true LRU.

- **2Q (Two-Queue)** – Uses two queues (FIFO + LRU) to resist scan pollution.  
  Protects cache from being flushed by large sequential scans.

- **SLRU (Segmented LRU)** – Splits cache into probationary and protected segments.  
  Promotes frequently reused items while allowing new data to be tested.

- **CAR (Clock with Adaptive Replacement)** – Combines Clock’s efficiency with ARC’s adaptivity.  
  Adaptive and scan-resistant, with lower overhead than ARC.

---

## Benchmark Results

The following results compare different **prefetching strategies** combined with an **LRU cache policy**,
using an **80/20 working set pattern** and a **cache size of 300**.

| Prefetch Strategy | Avg. Time (µs) | Std. Dev. (µs) | Slope (µs) |
|-------------------|----------------|----------------|------------|
| **Sequential**    | 65.21          | 3.06           | 64.88      |
| **None**          | 68.52          | 1.74           | 68.34      |
| **History-Based** | 252.36         | 4.94           | 252.31     |
| **Markov**        | 288.22         | 18.98          | 286.03     |
| **Adaptive**      | 667.24         | 21.93          | 667.21     |

![LRU Sequential Benchmark](https://github.com/roquess/fulgurance/blob/main/lruseq.png?raw=true)

---

## Benchmark Environment

All benchmarks were executed on the following system:

- **CPU**: AMD Ryzen 9 7900 (12-Core, 3.70 GHz)
- **RAM**: 64 GB (63.1 GB usable)

