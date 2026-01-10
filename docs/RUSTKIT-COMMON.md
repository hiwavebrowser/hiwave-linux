# RustKit Common

Common utilities, error types, and logging configuration for the RustKit browser engine.

## Overview

RustKit Common provides:
- **Unified error types** with backtrace support and categorization
- **Logging configuration** for development and production
- **Retry utilities** with exponential backoff
- **Result extension traits** for ergonomic error handling

## Error Handling

### RustKitError

The unified error type for all RustKit operations:

```rust
use rustkit_common::{RustKitError, Result};

fn example() -> Result<()> {
    // Create errors with context
    let err = RustKitError::view("Failed to create HWND");
    let err = RustKitError::network_with_source("Connection failed", io_error);
    let err = RustKitError::internal("Unexpected state"); // Includes backtrace
    
    // Check error properties
    if err.is_retryable() {
        // Network, timeout, and IO errors are retryable
    }
    
    // Get category for metrics
    let category = err.category(); // "view", "network", etc.
    
    Err(err)
}
```

### Error Categories

| Category | Retryable | Example |
|----------|-----------|---------|
| `view` | No | Window creation failed |
| `network` | Yes | Connection refused |
| `dom` | No | Invalid HTML |
| `javascript` | No | Script error |
| `render` | No | GPU error |
| `layout` | No | Invalid dimensions |
| `navigation` | No | Invalid URL |
| `config` | No | Missing setting |
| `io` | Yes | File not found |
| `timeout` | Yes | Operation timed out |
| `cancelled` | No | User cancelled |
| `not_found` | No | Resource missing |
| `invalid_argument` | No | Bad parameter |
| `internal` | No | Unexpected state |

### Result Extensions

```rust
use rustkit_common::{ResultExt, OptionExt};

// Add context to errors
let result = some_fallible_operation()
    .context("Failed to initialize engine")?;

// Convert Option to Result
let view = views.get(&id)
    .ok_or_not_found("view")?;
```

## Logging

### Quick Start

```rust
use rustkit_common::{init_logging, LogConfig};

// Development (pretty output)
init_logging(LogConfig::debug());

// Production (JSON output)
init_logging(LogConfig::production());

// Custom filter
init_logging(LogConfig::default().with_filter("rustkit=debug,wgpu=warn"));
```

### Configuration Options

```rust
use rustkit_common::{LogConfig, LogFormat};
use tracing::Level;

let config = LogConfig {
    level: Level::DEBUG,
    format: LogFormat::Pretty, // or Compact, Json
    include_location: true,    // file:line
    include_thread_names: false,
    include_span_events: true, // enter/exit events
    filter: Some("rustkit=debug".to_string()),
};

init_logging(config);
```

### Log Formats

```
# Pretty (default)
2026-01-02T16:00:00.000Z  INFO rustkit_engine: Engine initialized adapter="NVIDIA GeForce RTX 4090"

# Compact
2026-01-02T16:00:00Z  INFO rustkit_engine: Engine initialized

# JSON
{"timestamp":"2026-01-02T16:00:00Z","level":"INFO","target":"rustkit_engine","message":"Engine initialized","adapter":"NVIDIA GeForce RTX 4090"}
```

## Retry Utilities

### Basic Retry

```rust
use rustkit_common::{retry_with_backoff, RetryConfig};

let config = RetryConfig::default(); // 3 attempts, 100ms initial delay

let result = retry_with_backoff(&config, || async {
    fetch_resource().await
}).await?;
```

### Custom Configuration

```rust
use rustkit_common::RetryConfig;
use std::time::Duration;

let config = RetryConfig {
    max_attempts: 5,
    initial_delay: Duration::from_millis(50),
    max_delay: Duration::from_secs(30),
    backoff_multiplier: 2.0,
    jitter: true, // Prevent thundering herd
};
```

### Retry Patterns

| Pattern | Config |
|---------|--------|
| No retry | `RetryConfig::none()` |
| Default | `RetryConfig::default()` (3 attempts) |
| Aggressive | `RetryConfig::aggressive()` (5 attempts, faster) |

### Timeout

```rust
use rustkit_common::with_timeout;
use std::time::Duration;

let result = with_timeout(Duration::from_secs(30), || async {
    long_running_operation().await
}).await?;
```

## Best Practices

### Error Creation

```rust
// Prefer specific error types
return Err(RustKitError::navigation("Invalid URL scheme"));

// Add source errors when available
return Err(RustKitError::network_with_source("Request failed", reqwest_error));

// Use internal() for unexpected states (includes backtrace)
return Err(RustKitError::internal("View state corrupted"));
```

### Error Handling

```rust
match engine.load_url(view, url).await {
    Ok(()) => {}
    Err(e) if e.is_retryable() => {
        // Consider retry
    }
    Err(RustKitError::NotFound(resource)) => {
        // Handle 404
    }
    Err(e) => {
        // Log and propagate
        tracing::error!(category = e.category(), error = %e, "Operation failed");
        return Err(e);
    }
}
```

### Logging Levels

| Level | Use Case |
|-------|----------|
| `error` | Failures that affect functionality |
| `warn` | Recoverable issues, deprecations |
| `info` | Important state changes |
| `debug` | Detailed flow for debugging |
| `trace` | Very detailed, per-event logging |

## Testing

```bash
# Run common tests
cargo test -p rustkit-common

# With logging output
RUST_LOG=debug cargo test -p rustkit-common -- --nocapture
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: observability-hardening*

