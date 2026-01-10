# RustKit Performance

This document describes the performance benchmarking infrastructure for the RustKit browser engine.

## Overview

RustKit includes comprehensive benchmarking tools to:
- Measure parsing, styling, and layout performance
- Track performance regressions
- Compare against baseline measurements
- Generate performance reports

## Running Benchmarks

### Quick Benchmark

```bash
# Run all benchmarks with criterion
cargo bench -p rustkit-bench

# Run specific benchmark group
cargo bench -p rustkit-bench -- html_parsing
cargo bench -p rustkit-bench -- css_parsing
cargo bench -p rustkit-bench -- layout
```

### Programmatic Benchmark

```rust
use rustkit_bench::Benchmark;

let bench = Benchmark::new()
    .with_warmup(10)
    .with_iterations(100);

let suite = bench.run_all();
suite.print_summary();
suite.save_json("bench_results.json")?;
```

## Benchmark Categories

### HTML Parsing

| Benchmark | Description | Target |
|-----------|-------------|--------|
| `html/parse/small` | Single element document | < 1µs |
| `html/parse/medium` | 100 paragraphs (~3KB) | < 100µs |
| `html/parse/large` | 1000 paragraphs (~30KB) | < 1ms |

### CSS Parsing

| Benchmark | Description | Target |
|-----------|-------------|--------|
| `css/parse/small` | 2 rules | < 1µs |
| `css/parse/medium` | 50 rules (~2KB) | < 50µs |
| `css/parse/large` | 200 rules (~8KB) | < 200µs |

### Layout

| Benchmark | Description | Target |
|-----------|-------------|--------|
| `layout/simple` | Single block | < 500ns |
| `layout/nested_10` | 10 nested blocks | < 5µs |
| `layout/nested_100` | 100 nested blocks | < 50µs |

## Performance Targets

### MVP Targets (from WINCAIRO-LIMITATIONS)

| Metric | Target | Notes |
|--------|--------|-------|
| First Paint | < 500ms | From navigation start |
| DOM Ready | < 1s | DOMContentLoaded |
| Full Load | < 3s | Load event for typical page |
| Resize Latency | < 16ms | Maintain 60fps during resize |
| Memory (idle) | < 100MB | Per tab baseline |
| Memory (active) | < 500MB | Complex page limit |

### Parsing Throughput

| Format | Target | Rationale |
|--------|--------|-----------|
| HTML | > 10 MB/s | Keep up with fast networks |
| CSS | > 5 MB/s | Process large stylesheets quickly |

### Layout Performance

| Scenario | Target |
|----------|--------|
| Initial layout | < 50ms for typical page |
| Incremental | < 5ms per change |
| Resize relayout | < 16ms |

## Benchmark Results

### Sample Output

```
================================================================================
Benchmark Suite: RustKit Engine
================================================================================
Name                                           Mean       StdDev    Throughput
--------------------------------------------------------------------------------
html/parse/small (84 bytes)                  1.23 µs      ±0.12 µs     813.0K/s
html/parse/medium (3421 bytes)              45.67 µs      ±2.34 µs      21.9K/s
html/parse/large (34021 bytes)             423.45 µs     ±15.67 µs       2.4K/s
css/parse/small (55 bytes)                   0.89 µs      ±0.08 µs       1.1M/s
css/parse/medium (2150 bytes)               34.56 µs      ±1.23 µs      28.9K/s
layout/simple (1 box)                        0.45 µs      ±0.05 µs       2.2M/s
layout/nested (10 boxes)                     4.56 µs      ±0.34 µs     219.3K/s
--------------------------------------------------------------------------------
Total time: 1.234s
```

## Criterion Integration

Benchmarks use [Criterion.rs](https://github.com/bheisler/criterion.rs) for:
- Statistical analysis
- HTML reports
- Regression detection
- Throughput measurement

### HTML Reports

After running benchmarks, view HTML reports at:
```
target/criterion/report/index.html
```

## Performance Instrumentation

### Adding Instrumentation

```rust
use tracing::{debug, span, Level};

fn expensive_operation() {
    let span = span!(Level::DEBUG, "expensive_op");
    let _guard = span.enter();
    
    // ... operation ...
    
    debug!(elapsed_ms = start.elapsed().as_millis(), "Completed");
}
```

### Collecting Traces

```bash
# Enable tracing
RUST_LOG=rustkit=trace cargo run

# JSON output for analysis
RUST_LOG=rustkit=trace,trace_format=json cargo run
```

## Profiling

### CPU Profiling

```bash
# Windows (with cargo-flamegraph)
cargo install flamegraph
cargo flamegraph --bench rustkit

# Or with VS Performance Tools
devenv /ProfileVS hiwave.exe
```

### Memory Profiling

```bash
# Use DHAT for heap profiling
cargo build --release --features dhat
./target/release/hiwave
```

## Regression Testing

### CI Integration

```yaml
# .github/workflows/bench.yml
- name: Run Benchmarks
  run: cargo bench -p rustkit-bench -- --save-baseline main
  
- name: Compare to Baseline
  run: cargo bench -p rustkit-bench -- --baseline main
```

### Local Comparison

```bash
# Save baseline
cargo bench -- --save-baseline before

# Make changes...

# Compare
cargo bench -- --baseline before
```

## Custom Benchmarks

### Adding a New Benchmark

1. Add to `benches/rustkit.rs`:

```rust
fn my_benchmark(c: &mut Criterion) {
    c.bench_function("my_operation", |b| {
        b.iter(|| {
            // Operation to measure
        })
    });
}

criterion_group!(benches, my_benchmark);
```

2. Add to `rustkit-bench/src/lib.rs`:

```rust
fn bench_my_operation(&self) -> BenchmarkResult {
    self.run("my/operation", || {
        // Operation to measure
    })
}
```

## Best Practices

1. **Warmup**: Always include warmup iterations
2. **Samples**: Use enough samples for statistical significance (100+)
3. **Isolation**: Minimize allocations in hot paths
4. **Real Data**: Use representative test data
5. **Track Trends**: Monitor performance over time

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: perf-benchmarks*

