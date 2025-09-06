# Fulgurance

A blazing-fast, adaptive **prefetching and caching library for Rust**.  
Fulgurance optimizes memory and disk accesses by predicting and prefetching data before it’s needed, reducing latency and improving performance for **databases, distributed systems, and high-performance applications**.

---

## Features

- **Modular design** – Swap cache policies (**LRU, MRU, LFU, FIFO, Random**) and prefetching strategies (**None, Sequential, Markov, Stride, History-Based, Adaptive**).
- **Benchmark-ready** – Built with [criterion](https://crates.io/crates/criterion) for performance comparisons.
- **Zero-cost abstractions** – Leverages Rust’s performance for minimal overhead.
- **Extensible** – Easy to add custom policies or integrate with existing systems.

---

## Installation

Add Fulgurance to your `Cargo.toml`:

```toml
[dependencies]
fulgurance = "0.2.0"
```

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

## Benchmark Environment

All benchmarks were executed on the following system:

- **CPU**: AMD Ryzen 9 7900 (12-Core, 3.70 GHz)
- **RAM**: 64 GB (63.1 GB usable)


