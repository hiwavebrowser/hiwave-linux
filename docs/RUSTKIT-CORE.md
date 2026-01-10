# RustKit Core

The core engine runtime provides task scheduling, timers, and navigation state management for the RustKit browser engine.

## Overview

RustKit Core solves the **page load events** issue from WinCairo by providing:

1. **Deterministic navigation state machine**: start → commit → finish guaranteed
2. **Reliable event delivery**: Events emitted via channel, never dropped
3. **Timer system**: setTimeout/setInterval equivalents
4. **Task queue**: Priority-based task scheduling

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Engine Core                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │             NavigationStateMachine                   │    │
│  │  state: Idle → Provisional → Committed → Finished    │    │
│  │  history: Vec<Url>                                   │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                    TaskQueue                         │    │
│  │  priority levels: Idle < Normal < High < Critical    │    │
│  │  timers: HashMap<TimerId, TimerEntry>                │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Navigation State Machine

### States

| State | Description |
|-------|-------------|
| `Idle` | No navigation in progress |
| `Provisional` | Navigation started, waiting for response |
| `Committed` | First bytes received |
| `Finished` | Page fully loaded |
| `Failed` | Navigation failed |

### State Transitions

```
    ┌───────┐
    │ Idle  │
    └───┬───┘
        │ start_navigation()
        ▼
┌───────────────┐
│  Provisional  │
└───────┬───────┘
        │ commit_navigation()
        ▼
┌───────────────┐
│   Committed   │
└───────┬───────┘
        │ finish_navigation()
        ▼
┌───────────────┐       ┌────────┐
│   Finished    │──────▶│  Idle  │
└───────────────┘       └────────┘

        │ fail_navigation() (from any state)
        ▼
┌───────────────┐
│    Failed     │──────▶ Idle
└───────────────┘
```

### Events

```rust
pub enum LoadEvent {
    DidStartProvisionalLoad { navigation_id, url },
    DidCommitLoad { navigation_id, url },
    DidFinishLoad { navigation_id, url },
    DidFailLoad { navigation_id, url, error },
    DidUpdateProgress { navigation_id, progress },
}
```

### Usage

```rust
use rustkit_core::{NavigationStateMachine, NavigationRequest, LoadEvent};
use tokio::sync::mpsc;
use url::Url;

// Create event channel
let (tx, mut rx) = mpsc::unbounded_channel();
let mut nav = NavigationStateMachine::new(tx);

// Start navigation
let url = Url::parse("https://example.com")?;
let request = NavigationRequest::new(url);
let nav_id = nav.start_navigation(request)?;
// Event: DidStartProvisionalLoad

// Network: first bytes received
nav.commit_navigation()?;
// Event: DidCommitLoad

// Progress updates during load
nav.update_progress(0.5)?;
// Event: DidUpdateProgress

// Page fully loaded
nav.finish_navigation()?;
// Event: DidFinishLoad

// Handle events
while let Some(event) = rx.recv().await {
    match event {
        LoadEvent::DidStartProvisionalLoad { .. } => {
            // Show loading indicator
        }
        LoadEvent::DidFinishLoad { .. } => {
            // Hide loading indicator
        }
        _ => {}
    }
}
```

## Task Queue

### Priority Levels

| Priority | Use Case |
|----------|----------|
| `Critical` | Error handling, security |
| `High` | User input, navigation |
| `Normal` | Script execution, rendering |
| `Idle` | Analytics, prefetch |

### Usage

```rust
use rustkit_core::{TaskQueue, TaskPriority};

let (queue, mut receiver) = TaskQueue::new();

// Post a task
queue.post_task(TaskPriority::Normal, Box::new(|| {
    println!("Task executed!");
}))?;

// Process tasks
while let Some((priority, task)) = receiver.recv().await {
    task();
}
```

## Timers

### setTimeout Equivalent

```rust
let timer_id = queue.set_timeout(|| {
    println!("Fired after 100ms");
}, Duration::from_millis(100));

// Cancel if needed
queue.clear_timer(timer_id);
```

### setInterval Equivalent

```rust
let interval_id = queue.set_interval(|| {
    println!("Fires every 1000ms");
}, Duration::from_secs(1));

// Cancel when done
queue.clear_timer(interval_id);
```

## History Navigation

```rust
// Check navigation availability
if nav.can_go_back() {
    let url = nav.go_back();
    // Start navigation to url
}

if nav.can_go_forward() {
    let url = nav.go_forward();
    // Start navigation to url
}

// Current URL
let current = nav.current_url();
```

## Solving WinCairo Issues

### Issue: Page Load Events Not Implemented

**WinCairo problem**: `WKPageLoaderClient` callbacks not firing.

**Solution**:
- `NavigationStateMachine` explicitly manages state transitions
- Events sent via `mpsc::unbounded_channel` - guaranteed delivery
- State must transition through: Provisional → Committed → Finished
- Invalid transitions return errors (no silent failures)

### Deterministic Event Ordering

Events are **always** emitted in this order:
1. `DidStartProvisionalLoad`
2. `DidUpdateProgress` (0 or more times)
3. `DidCommitLoad`
4. `DidUpdateProgress` (0 or more times)
5. `DidFinishLoad` OR `DidFailLoad`

This matches the WebKit specification exactly.

## Integration Example

```rust
use rustkit_core::{NavigationStateMachine, NavigationRequest, LoadEvent};
use rustkit_viewhost::ViewHost;
use rustkit_compositor::Compositor;

// Create components
let mut host = ViewHost::new();
let compositor = Compositor::new()?;
let (tx, mut rx) = mpsc::unbounded_channel();
let mut nav = NavigationStateMachine::new(tx);

// Create view
let view_id = host.create_view(parent, bounds)?;

// Handle load events
tokio::spawn(async move {
    while let Some(event) = rx.recv().await {
        match event {
            LoadEvent::DidStartProvisionalLoad { url, .. } => {
                // Update UI: loading started
            }
            LoadEvent::DidUpdateProgress { progress, .. } => {
                // Update progress bar
            }
            LoadEvent::DidFinishLoad { url, .. } => {
                // Update UI: loading complete
            }
            LoadEvent::DidFailLoad { error, .. } => {
                // Show error page
            }
            _ => {}
        }
    }
});

// Navigate
let url = Url::parse("https://example.com")?;
nav.start_navigation(NavigationRequest::new(url))?;
```

## Testing

```bash
# Run unit tests
cargo test -p rustkit-core

# Run with logging
RUST_LOG=rustkit_core=debug cargo test -p rustkit-core
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: engine-scheduler-core*

