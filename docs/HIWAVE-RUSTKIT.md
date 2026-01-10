# HiWave RustKit Integration

This document describes how to use the RustKit browser engine as an alternative backend in HiWave.

## Overview

RustKit is a pure-Rust browser engine that provides:
- Win32 window hosting (rustkit-viewhost)
- GPU rendering via wgpu (rustkit-compositor)
- HTML parsing (rustkit-dom)
- CSS parsing and styling (rustkit-css)
- Block/inline layout (rustkit-layout)
- JavaScript execution via Boa (rustkit-js)
- HTTP networking (rustkit-net)
- Full engine orchestration (rustkit-engine)

## Building with RustKit

### Enable the Feature

```bash
# Build with RustKit backend
cargo build --features rustkit

# Run with RustKit
cargo run --features rustkit
```

### Feature Flags

| Feature | Backend | Description |
|---------|---------|-------------|
| (default) | WRY/WebView2 | Uses Microsoft Edge WebView2 |
| `wincairo` | WinCairo WebKit | Uses Apple WebKit (WinCairo port) |
| `rustkit` | RustKit | Uses pure-Rust engine |

Note: `rustkit` takes precedence over `wincairo` if both are enabled.

## Architecture

### Three-WebView Layout

HiWave uses three WebViews:

```
┌─────────────────────────────────────────────────────────────┐
│                     Chrome WebView                           │
│  (tabs, toolbar, sidebar - always WRY/WebView2)             │
├─────────────────────────────────────────────────────────────┤
│  Sidebar  │                                                  │
│  (part of │              Content WebView                     │
│  chrome)  │      (web pages - can use RustKit)              │
│           │                                                  │
├───────────┴─────────────────────────────────────────────────┤
│                     Shelf WebView                            │
│           (command palette - always WRY/WebView2)           │
└─────────────────────────────────────────────────────────────┘
```

When RustKit is enabled, only the **Content WebView** uses the RustKit engine. The Chrome and Shelf continue to use WRY/WebView2 for stability.

### Component Integration

```
HiWave App
    │
    ├── webview.rs (IWebView trait)
    │       ├── wry::WebView (default)
    │       ├── webkit_wincairo::WebKitView (wincairo)
    │       └── RustKitView (rustkit)
    │
    └── webview_rustkit.rs (rustkit feature)
            │
            └── rustkit_engine::Engine
                    ├── rustkit-viewhost
                    ├── rustkit-compositor
                    ├── rustkit-dom
                    ├── rustkit-css
                    ├── rustkit-layout
                    ├── rustkit-js
                    └── rustkit-net
```

## RustKitView API

```rust
use crate::webview_rustkit::{RustKitView, Bounds};

// Create a view
let view = RustKitView::new(parent_hwnd, Bounds::new(0, 0, 800, 600))?;

// Load a URL (async, non-blocking)
view.load_url("https://example.com");

// Load HTML directly
view.load_html("<html><body>Hello!</body></html>");

// Execute JavaScript
view.evaluate_script("document.title");

// Resize
view.set_bounds(Rect { ... });

// Get current URL
let url = view.url();
```

## IWebView Trait

All backends implement the same `IWebView` trait:

```rust
pub trait IWebView {
    fn load_url(&self, url: &str);
    fn load_html(&self, html: &str);
    fn evaluate_script(&self, script: &str);
    fn set_bounds(&self, rect: Rect);
    fn url(&self) -> Option<String>;
    fn set_zoom(&self, level: f64);
    fn print(&self);
    fn focus(&self);
    fn clear_all_browsing_data(&self);
    fn set_visible(&self, visible: bool);
}
```

## Engine Configuration

The RustKit engine is configured in `webview_rustkit.rs`:

```rust
let engine = EngineBuilder::new()
    .user_agent("HiWave/1.0 RustKit/1.0")
    .javascript_enabled(true)
    .cookies_enabled(true)
    .build()?;
```

## Current Limitations

RustKit is under active development. Current limitations:

| Feature | Status |
|---------|--------|
| HTML rendering | ✅ Basic |
| CSS styling | ✅ Basic |
| JavaScript | ✅ Via Boa engine |
| Networking | ✅ fetch, downloads |
| Print | ❌ Not implemented |
| Zoom | ❌ Not implemented |
| DevTools | ❌ Not available |
| Extensions | ❌ Not supported |

## Debugging

### Enable Tracing

```rust
// In code
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .with_env_filter("rustkit=debug")
    .init();
```

Or via environment:

```bash
RUST_LOG=rustkit=debug cargo run --features rustkit
```

### Check Engine Status

```rust
use crate::webview::engine_name;

println!("Using engine: {}", engine_name());
// Output: "RustKit" when rustkit feature is enabled
```

## Workspace Dependencies

All RustKit crates are available as workspace dependencies:

```toml
[dependencies]
rustkit-engine = { workspace = true }
rustkit-viewhost = { workspace = true }
rustkit-compositor = { workspace = true }
rustkit-core = { workspace = true }
rustkit-dom = { workspace = true }
rustkit-css = { workspace = true }
rustkit-layout = { workspace = true }
rustkit-js = { workspace = true }
rustkit-bindings = { workspace = true }
rustkit-net = { workspace = true }
rustkit-common = { workspace = true }
```

## Testing

```bash
# Test RustKit crates
cargo test -p rustkit-engine
cargo test -p rustkit-dom
cargo test --workspace --features rustkit

# Build and run smoke test
cargo run -p hiwave-smoke
```

## Future Work

- [ ] Implement zoom support
- [ ] Add print functionality
- [ ] Improve CSS compatibility
- [ ] Add DevTools integration
- [ ] WebAssembly support
- [ ] Service worker support

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: hiwave-integration*

