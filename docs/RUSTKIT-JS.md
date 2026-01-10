# RustKit JS

JavaScript engine integration for the RustKit browser engine.

## Overview

RustKit JS provides:
- **Engine abstraction**: Pluggable JS engine backend (Boa, V8)
- **Web API compatibility**: console, setTimeout, setInterval
- **Safe interop**: Controlled Rust ↔ JS boundary
- **Async support**: Timer and event loop integration

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      JsRuntime                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Engine Backend (Boa / V8)                           │    │
│  │  - Script evaluation                                 │    │
│  │  - Value conversion                                  │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Web APIs                                            │    │
│  │  - console.log/warn/error                            │    │
│  │  - setTimeout / setInterval                          │    │
│  │  - Global variables                                  │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Timer System                                        │    │
│  │  - Pending timers                                    │    │
│  │  - Event loop integration                            │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Usage

### Basic Script Evaluation

```rust
use rustkit_js::{JsRuntime, JsValue};

let mut runtime = JsRuntime::new()?;

// Evaluate expressions
let result = runtime.evaluate_script("1 + 2")?;
assert!(matches!(result, JsValue::Number(3.0)));

// Define and call functions
runtime.evaluate_script("function greet(name) { return 'Hello, ' + name; }")?;
let greeting = runtime.evaluate_script("greet('World')")?;
```

### Global Variables

```rust
use rustkit_js::{JsRuntime, JsValue};

let mut runtime = JsRuntime::new()?;

// Set a global
runtime.set_global("appName", JsValue::String("HiWave".into()))?;

// Use in script
let result = runtime.evaluate_script("'Welcome to ' + appName")?;

// Check existence
if runtime.has_global("appName") {
    let value = runtime.get_global("appName")?;
}
```

### Console API

```rust
use rustkit_js::{JsRuntime, LogLevel};

let mut runtime = JsRuntime::new()?;

// Set up console handler
runtime.set_console_handler(Box::new(|level, msg| {
    match level {
        LogLevel::Log => println!("[LOG] {}", msg),
        LogLevel::Warn => println!("[WARN] {}", msg),
        LogLevel::Error => eprintln!("[ERROR] {}", msg),
        _ => {}
    }
}));

// Use console in scripts
runtime.evaluate_script("console.log('Hello from JS!')")?;
runtime.evaluate_script("console.warn('This is a warning')")?;
```

### Timers

```rust
use rustkit_js::JsRuntime;

let mut runtime = JsRuntime::new()?;

// setTimeout equivalent
let timeout_id = runtime.set_timeout("console.log('Delayed!')", 1000);

// setInterval equivalent
let interval_id = runtime.set_interval("console.log('Repeating!')", 500);

// Cancel timers
runtime.clear_timer(timeout_id);
runtime.clear_timer(interval_id);

// Execute due timers in event loop
for (id, _code, _repeat) in runtime.get_due_timers() {
    runtime.execute_timer(id)?;
}
```

## JsValue Types

| Type | Description |
|------|-------------|
| `Undefined` | JavaScript `undefined` |
| `Null` | JavaScript `null` |
| `Boolean(bool)` | Boolean value |
| `Number(f64)` | Numeric value (all JS numbers are f64) |
| `String(String)` | String value |
| `Object` | Generic object |
| `Array` | Array object |
| `Function` | Function object |

## Engine Backends

### Boa (Default)

Pure Rust JavaScript engine. Good for development and testing.

```toml
[dependencies]
rustkit-js = { path = "../rustkit-js" }  # Uses boa by default
```

Advantages:
- Pure Rust, easy to build
- No native dependencies
- Good ES2020 support

### V8 (Production)

Google's V8 engine via rusty_v8. Recommended for production.

```toml
[dependencies]
rustkit-js = { path = "../rustkit-js", features = ["v8"], default-features = false }
```

Advantages:
- Production-proven performance
- Full ES2023 support
- JIT compilation

## Error Handling

```rust
use rustkit_js::{JsRuntime, JsError};

let mut runtime = JsRuntime::new()?;

match runtime.evaluate_script("invalid syntax {{{") {
    Ok(_) => { /* success */ }
    Err(JsError::ParseError(msg)) => {
        eprintln!("Syntax error: {}", msg);
    }
    Err(JsError::ExecutionError(msg)) => {
        eprintln!("Runtime error: {}", msg);
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## Integration with DOM

```rust
use rustkit_js::JsRuntime;
use rustkit_dom::Document;

let doc = Document::parse_html("<html><body><p id='msg'>Hello</p></body></html>")?;
let mut runtime = JsRuntime::new()?;

// Inject document reference (via bindings crate)
// runtime.bind_document(&doc);

// Now JS can access the DOM
// runtime.evaluate_script("document.getElementById('msg').textContent")?;
```

## Event Loop Integration

```rust
use rustkit_js::JsRuntime;
use std::time::{Duration, Instant};

let mut runtime = JsRuntime::new()?;

// Schedule some timers
runtime.set_timeout("result = 'done'", 100);

// Event loop
let start = Instant::now();
loop {
    // Check for due timers
    for (id, _code, _repeat) in runtime.get_due_timers() {
        runtime.execute_timer(id)?;
    }
    
    // Exit condition
    if start.elapsed() > Duration::from_millis(200) {
        break;
    }
    
    std::thread::sleep(Duration::from_millis(10));
}
```

## Performance Considerations

1. **Script caching**: Avoid re-parsing the same script
2. **Batch operations**: Group multiple evaluations
3. **Minimize crossing**: Reduce Rust ↔ JS boundary crossings
4. **Use native functions**: Implement hot paths in Rust

## Testing

```bash
# Run JS tests
cargo test -p rustkit-js

# With logging
RUST_LOG=rustkit_js=debug cargo test -p rustkit-js
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: js-engine-integration*

