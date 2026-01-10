# Migration from WinCairo WebKit to RustKit

This document describes the migration from WebKit's WinCairo port to the pure-Rust RustKit browser engine.

## Overview

HiWave originally used WebKit's WinCairo port as an alternative to WebView2/Chromium for content rendering. However, due to the limitations documented in `WINCAIRO-LIMITATIONS.md`, we developed RustKit as a replacement.

## Changes Summary

### Removed Components

- `crates/webkit-wincairo-sys/` - FFI bindings to WebKit C API
- `crates/webkit-wincairo/` - Rust wrapper around WebKit

### Added Components

- `crates/rustkit-*` - 13 crates comprising the RustKit browser engine

### Feature Flag Changes

| Before | After |
|--------|-------|
| `--features wincairo` | `--features rustkit` |

### API Changes

#### webview.rs

```rust
// Before
#[cfg(all(target_os = "windows", feature = "wincairo"))]
pub type HiWaveWebView = webkit_wincairo::WebKitView;

// After
// HiWaveWebView is always wry::WebView (for Chrome/Shelf/Settings)
// RustKit is used via webview_rustkit module for content
pub type HiWaveWebView = wry::WebView;
```

#### WebViewEngine enum

```rust
// Before
pub enum WebViewEngine {
    WebKit,
    WebView2,
    WinCairoWebKit,  // Removed
}

// After
pub enum WebViewEngine {
    WebKit,
    WebView2,
    RustKit,  // New
}
```

## Build Commands

```bash
# Default (WebView2)
cargo build

# With RustKit content rendering
cargo build --features rustkit

# Run with RustKit
cargo run --features rustkit
```

## Benefits of RustKit over WinCairo

| Issue | WinCairo Status | RustKit Status |
|-------|-----------------|----------------|
| View resize | ❌ Broken | ✅ Works |
| Multiple WebViews | ⚠️ Partial | ✅ Full support |
| Page load events | ❌ Missing | ✅ Implemented |
| Navigation interception | ⚠️ Limited | ✅ Full control |
| Download handling | ❌ Missing | ✅ Download manager |
| Memory management | ⚠️ C++ ownership | ✅ Rust ownership |
| Thread safety | ⚠️ Manual | ✅ Compile-time |
| Build complexity | ❌ Complex | ✅ cargo build |

## RustKit Architecture

```
rustkit-engine (orchestration)
├── rustkit-viewhost (Win32 windows)
├── rustkit-compositor (GPU rendering)
├── rustkit-core (task scheduler)
├── rustkit-dom (HTML parsing)
├── rustkit-css (CSS parsing)
├── rustkit-layout (layout engine)
├── rustkit-js (JavaScript - Boa)
├── rustkit-bindings (DOM API)
├── rustkit-net (networking)
└── rustkit-common (utilities)
```

## Migration Steps (if updating existing code)

1. Remove `wincairo` feature from your Cargo.toml
2. Add `rustkit` feature if you want non-WebView2 rendering
3. Update any code using `webkit_wincairo::*` types
4. Replace `WebViewEngine::WinCairoWebKit` with `WebViewEngine::RustKit`
5. Test with `cargo build --features rustkit`

## Deprecation Timeline

- **Phase 11**: WinCairo code removed from repository
- **Future**: WinCairo references removed from documentation

## Related Documents

- `docs/WINCAIRO-LIMITATIONS.md` - Original limitations that motivated RustKit
- `docs/HIWAVE-RUSTKIT.md` - RustKit integration guide
- `docs/RUSTKIT-*.md` - Individual crate documentation

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: wincairo-removal*

