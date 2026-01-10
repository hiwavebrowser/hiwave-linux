# Shield (Ad/Tracker Blocking) Architecture

This document explains how HiWave's ad and tracker blocking system works, particularly the integration between the main shield and RustKit's network layer.

## Overview

HiWave uses a **two-layer blocking architecture**:

1. **Navigation-level blocking** (main shield) - Blocks page navigations and popups
2. **Sub-resource blocking** (RustKit interceptor) - Blocks scripts, images, XHR, etc.

```
┌─────────────────────────────────────────────────────────────┐
│                     User Request                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: Navigation Handler (main.rs)                       │
│  ─────────────────────────────────────────                   │
│  • Runs on main thread                                       │
│  • Uses Brave's adblock-rust engine (full EasyList)          │
│  • Blocks: Page loads, popups, new windows                   │
│  • Updates: shield.requests_blocked counter                  │
│  • NOT Send+Sync (uses Rc<RefCell> internally)               │
└─────────────────────────────────────────────────────────────┘
                              │
                    (if navigation allowed)
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 2: RustKit Request Interceptor (shield_adapter.rs)    │
│  ─────────────────────────────────────────────────────────   │
│  • Runs on async network thread                              │
│  • Uses simple domain-based filter                           │
│  • Blocks: Scripts, images, XHR, CSS, fonts                  │
│  • Updates: AtomicU64 counter (thread-safe)                  │
│  • IS Send+Sync (required for async)                         │
└─────────────────────────────────────────────────────────────┘
```

## Why Two Layers?

### The Problem

Brave's `adblock-rust` engine provides excellent blocking using full EasyList/EasyPrivacy filter lists. However, it uses `Rc<RefCell>` internally for performance reasons, making it **not thread-safe**.

RustKit's network layer (`rustkit-net`) runs asynchronously and requires request handlers to be `Send + Sync`.

### The Solution

We use two complementary systems:

| Aspect | Main Shield | RustKit Interceptor |
|--------|-------------|---------------------|
| **Location** | `hiwave-shield` crate | `shield_adapter.rs` |
| **Engine** | Brave's adblock-rust | Simple domain matcher |
| **Filter Rules** | Full EasyList + EasyPrivacy (~70k rules) | ~30 common domains |
| **Thread Safety** | NOT Send+Sync | Send+Sync |
| **Blocks** | Navigations, popups | Sub-resources |
| **Counter** | `AtomicU64` in `AdBlocker` | Shared `Arc<AtomicU64>` |

## Data Flow

### 1. Page Navigation (Layer 1)

```rust
// In main.rs navigation handler
if shield.should_block(&url, &source, ResourceType::Document) {
    shield.increment_block_count();
    return false; // Block navigation
}
```

### 2. Sub-Resource Requests (Layer 2)

```rust
// In shield_adapter.rs
impl InterceptHandler for ShieldInterceptHandler {
    fn intercept(&self, request: &Request) -> InterceptAction {
        if self.should_block_host(request.url.host_str()) {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            return InterceptAction::Block;
        }
        InterceptAction::Allow
    }
}
```

## Blocked Domains (Layer 2)

The RustKit interceptor blocks these common ad/tracker domains:

```
Ad Networks:
- doubleclick.net
- googlesyndication.com
- googleadservices.com
- adnxs.com
- adsrvr.org
- adroll.com
- taboola.com
- outbrain.com
- rubiconproject.com
- openx.net
- pubmatic.com

Analytics/Tracking:
- scorecardresearch.com
- chartbeat.com
- segment.io / segment.com
- mixpanel.com
- hotjar.com
- fullstory.com
- googletagmanager.com

Social:
- ads.twitter.com
- facebook.com/tr
- connect.facebook.net
- tr.snapchat.com
- amazon-adsystem.com
- criteo.com
```

## Counter Synchronization

Both layers increment counters that contribute to the shield stats:

```rust
// Main shield (navigation blocks)
pub struct AdBlocker {
    requests_blocked: AtomicU64,  // Incremented for nav blocks
    trackers_blocked: AtomicU64,
}

// RustKit interceptor (sub-resource blocks)
pub struct ShieldInterceptHandler {
    blocked_count: Arc<AtomicU64>,  // Shared with view creator
}
```

To get total blocked requests, the app can sum both counters or use a shared counter.

## Integration Points

### Creating a RustKit View with Shield

```rust
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

// Create a shared counter
let blocked_counter = Arc::new(AtomicU64::new(0));

// Create view with shield integration
let view = RustKitView::with_shield_counter(
    parent_hwnd,
    bounds,
    Some(Arc::clone(&blocked_counter)),
)?;

// Later, read the count
let sub_resource_blocks = blocked_counter.load(Ordering::Relaxed);
```

### Engine Builder API

```rust
let interceptor = create_shield_interceptor_with_counter(counter);

let engine = EngineBuilder::new()
    .user_agent("HiWave/1.0")
    .request_interceptor(interceptor)
    .build()?;
```

## Future Improvements

1. **Sync counters to main shield** - Add sub-resource blocks to the main shield stats
2. **Rule synchronization** - Pull domain list from main shield filter lists
3. **Thread-safe adblock** - Investigate making adblock-rust Send+Sync
4. **Shared state channel** - Use channels to query main shield from async context

## Files

| File | Purpose |
|------|---------|
| `crates/hiwave-shield/src/lib.rs` | Main shield with Brave's adblock engine |
| `crates/hiwave-app/src/shield_adapter.rs` | RustKit network interceptor |
| `crates/rustkit-net/src/intercept.rs` | Interceptor trait and types |
| `crates/rustkit-net/src/lib.rs` | ResourceLoader with interceptor support |
| `crates/rustkit-engine/src/lib.rs` | Engine builder with interceptor wiring |
