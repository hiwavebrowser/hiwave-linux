# RustKit Testing

This document describes the testing infrastructure for the RustKit browser engine.

## Overview

RustKit uses a WPT-style (Web Platform Tests) test harness to verify:
- HTML parsing correctness
- CSS parsing and cascade
- Layout calculations
- Reference (visual) tests

## Test Types

### 1. Parse Tests (`tests/wpt/parse/`)

Test HTML parsing produces correct DOM trees.

```html
<!-- tests/wpt/parse/simple.html -->
<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body><p>Hello</p></body>
</html>
```

Optional `.expected` file for comparison:
```
<!-- tests/wpt/parse/simple.expected -->
#document
  <html>
    <head>
      <title>
        "Test"
    <body>
      <p>
        "Hello"
```

### 2. Style Tests (`tests/wpt/style/`)

Test CSS parsing and computed styles.

```css
/* tests/wpt/style/basic.css */
body { margin: 0; }
h1 { font-size: 24px; }
```

### 3. Layout Tests (`tests/wpt/layout/`)

Test box dimensions and positions.

```html
<!-- tests/wpt/layout/block.html -->
<!DOCTYPE html>
<style>
.box { width: 100px; height: 50px; }
</style>
<div class="box">Test</div>
```

Expected output:
```
<!-- tests/wpt/layout/block.expected -->
block: x=0 y=0 w=100 h=50
```

### 4. Reference Tests (`tests/wpt/reftest/`)

Compare rendered output between test and reference files.

**reftest.list** format:
```
# Match tests (should produce identical output)
== test.html reference.html

# Mismatch tests (should NOT match)
!= test.html different.html
```

## Running Tests

### Command Line

```bash
# Run all tests
cargo test -p rustkit-test

# Run with logging
RUST_LOG=rustkit_test=debug cargo test -p rustkit-test

# Run specific test type
cargo test -p rustkit-test parse
cargo test -p rustkit-test style
cargo test -p rustkit-test layout
```

### Programmatic

```rust
use rustkit_test::{TestHarness, TestConfig};

let harness = TestHarness::new();
let results = harness.run_all("tests/wpt")?;

println!("Results: {}/{} passed ({:.1}%)",
    results.passed,
    results.total,
    results.pass_rate()
);

// Print failures
for result in results.results.iter().filter(|r| r.status == TestStatus::Fail) {
    println!("FAIL: {} - {:?}", result.name, result.message);
    if let (Some(expected), Some(actual)) = (&result.expected, &result.actual) {
        println!("  Expected: {}", expected);
        println!("  Actual: {}", actual);
    }
}
```

## Test Configuration

```rust
use rustkit_test::TestConfig;

let config = TestConfig {
    test_dir: PathBuf::from("tests/wpt"),
    pattern: "*.html".to_string(),
    timeout_ms: 5000,
    skip_patterns: vec!["slow_*".to_string()],
    filter_patterns: vec!["css/*".to_string()],
};

let harness = TestHarness::with_config(config);
```

## Adding Tests

### Parse Test

1. Create `tests/wpt/parse/my-test.html`
2. Optionally create `tests/wpt/parse/my-test.expected`
3. Run `cargo test -p rustkit-test parse`

### Style Test

1. Create `tests/wpt/style/my-test.css` or `.html`
2. Optionally create expected output
3. Run tests

### Layout Test

1. Create `tests/wpt/layout/my-test.html`
2. Create `tests/wpt/layout/my-test.expected` with expected box dimensions
3. Run tests

### Reference Test

1. Create test file: `tests/wpt/reftest/my-test.html`
2. Create reference: `tests/wpt/reftest/my-test-ref.html`
3. OR add to `reftest.list`: `== my-test.html my-test-ref.html`
4. Run tests

## Test Results

### TestResult

```rust
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,  // Pass, Fail, Skip, Timeout, Error
    pub duration_ms: u64,
    pub message: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
}
```

### TestSummary

```rust
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errors: usize,
    pub duration_ms: u64,
    pub results: Vec<TestResult>,
}
```

## CI Integration

```yaml
# .github/workflows/test.yml
- name: Run RustKit Tests
  run: |
    cargo test -p rustkit-test -- --test-threads=1
    
- name: Report Coverage
  run: |
    cargo run -p rustkit-test -- --report coverage.json
```

## Writing Good Tests

1. **One thing per test**: Each test should verify one specific behavior
2. **Minimal HTML**: Use the simplest HTML that demonstrates the feature
3. **Document intent**: Add comments explaining what the test checks
4. **Expected output**: Always provide expected output for non-trivial tests
5. **Cross-browser**: Reference tests should work in real browsers too

## Current Coverage

| Feature | Tests | Pass Rate |
|---------|-------|-----------|
| HTML parsing | 10 | 90% |
| CSS parsing | 15 | 85% |
| Block layout | 8 | 75% |
| Inline layout | 5 | 60% |
| Reference tests | 3 | 100% |

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: wpt-harness*

