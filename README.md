# Fulgurance

**A blazing-fast, adaptive prefetching and caching library for Rust.**

Fulgurance optimizes memory and disk accesses by **predicting and prefetching data** before it's needed, reducing latency and improving performance for databases, distributed systems, and high-performance applications.

---

## Features
**Modular design**: Swap cache policies (LRU, FIFO, etc.) and prefetch strategies (stride, history-based, adaptive).
**Benchmark-ready**: Built-in support for `criterion` to compare strategies.
**Zero-cost abstractions**: Leverages Rust’s performance for minimal overhead.
**Extensible**: Easy to add custom policies or integrate with existing systems.

---

## Quick Start
Add Fulgurance to your `Cargo.toml`:
```toml
[dependencies]
fulgurance = { git = "https://github.com/roquess/fulgurance" }
