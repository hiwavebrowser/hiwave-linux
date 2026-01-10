# RustKit ViewHost

The ViewHost is the foundational layer of the RustKit browser engine that manages Win32 child windows (HWNDs) for hosting browser views.

## Overview

The ViewHost solves the **multi-view** and **resize** problems from WinCairo by:

1. **Per-view state isolation**: Each view has its own `ViewId` → `ViewState` mapping
2. **Direct resize handling**: `WM_SIZE` messages are processed immediately
3. **DPI awareness**: Per-monitor DPI scaling with `WM_DPICHANGED` handling
4. **No global singletons**: Multiple views can coexist without conflicts

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        ViewHost                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │             views: HashMap<ViewId, ViewState>        │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │          hwnd_to_view: HashMap<HWND, ViewId>         │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         ▼                    ▼                    ▼
    ┌─────────┐          ┌─────────┐          ┌─────────┐
    │ View 1  │          │ View 2  │          │ View 3  │
    │ (HWND)  │          │ (HWND)  │          │ (HWND)  │
    │ Chrome  │          │ Content │          │ Shelf   │
    └─────────┘          └─────────┘          └─────────┘
```

## API Reference

### ViewId

Unique identifier for each view.

```rust
let view_id = ViewId::new();
println!("View ID: {}", view_id.raw());
```

### Bounds

Rectangle defining view position and size.

```rust
let bounds = Bounds::new(x, y, width, height);
let zero = Bounds::zero();
```

### ViewHost

Main manager for all views.

```rust
use rustkit_viewhost::{ViewHost, Bounds};

// Create host
let mut host = ViewHost::new();

// Set event callback
host.set_event_callback(Box::new(|event| {
    match event {
        ViewEvent::Resized { view_id, bounds, dpi } => {
            // Notify compositor of resize
        }
        ViewEvent::Focused { view_id } => {
            // Update focus state
        }
        _ => {}
    }
}));

// Create a view
let bounds = Bounds::new(0, 0, 800, 600);
let view_id = host.create_view(parent_hwnd, bounds)?;

// Update bounds (handles resize correctly)
host.set_bounds(view_id, Bounds::new(0, 0, 1024, 768))?;

// Visibility
host.set_visible(view_id, true)?;

// Focus
host.focus(view_id)?;

// Get HWND for compositor
let hwnd = host.get_hwnd(view_id)?;

// Cleanup
host.destroy_view(view_id)?;
```

## Window Messages Handled

| Message | Handling |
|---------|----------|
| `WM_SIZE` | Emits `ViewEvent::Resized`, compositor resizes surface |
| `WM_DPICHANGED` | Updates view DPI, repositions window to suggested rect |
| `WM_SETFOCUS` | Emits `ViewEvent::Focused` |
| `WM_KILLFOCUS` | Emits `ViewEvent::Blurred` |
| `WM_PAINT` | Delegates to compositor via BeginPaint/EndPaint |
| `WM_ERASEBKGND` | Returns 1 to prevent flicker |

## Integration with Compositor

The ViewHost provides HWNDs that the compositor uses for rendering:

```rust
// ViewHost creates HWND
let view_id = host.create_view(parent, bounds)?;
let hwnd = host.get_hwnd(view_id)?;

// Compositor creates surface for HWND
let surface = compositor.create_surface_for_hwnd(hwnd, bounds.width, bounds.height)?;

// On resize, ViewHost notifies compositor
host.set_event_callback(Box::new(move |event| {
    if let ViewEvent::Resized { view_id, bounds, .. } = event {
        compositor.resize_surface(view_id, bounds.width, bounds.height);
    }
}));
```

## Solving WinCairo Issues

### Issue 1: Resize Not Updating Content

**WinCairo problem**: `SetWindowPos()` changes HWND size but compositor surface stays fixed.

**ViewHost solution**:
- `set_bounds()` calls `SetWindowPos()` + `InvalidateRect()`
- `WM_SIZE` handler emits `ViewEvent::Resized`
- Compositor receives event and recreates swapchain

### Issue 2: Multiple Views Fail

**WinCairo problem**: Global surface state causes last-created view to "steal" rendering.

**ViewHost solution**:
- Each view has independent `ViewId` → `ViewState`
- `hwnd_to_view` map enables per-view window proc handling
- Compositor maintains per-view `SurfaceState` keyed by `ViewId`

## Thread Safety

- `ViewHost` uses `RwLock<HashMap>` for view storage
- `ViewState` wrapped in `Arc<Mutex>` for interior mutability
- Event callback is `Send + Sync`
- HWND operations must occur on the window thread

## Error Handling

```rust
use rustkit_viewhost::ViewHostError;

match host.create_view(parent, bounds) {
    Ok(view_id) => { /* success */ }
    Err(ViewHostError::InvalidParent) => { /* null HWND */ }
    Err(ViewHostError::WindowCreation(msg)) => { /* CreateWindowEx failed */ }
    Err(e) => { /* other errors */ }
}
```

## Performance Considerations

1. **Minimize lock contention**: Read locks for queries, write locks only for mutations
2. **Batch operations**: Group multiple `set_bounds` calls when possible
3. **Async events**: Event callback runs synchronously; keep handlers fast

## Testing

```bash
# Run unit tests
cargo test -p rustkit-viewhost

# Run with tracing enabled
RUST_LOG=rustkit_viewhost=trace cargo test -p rustkit-viewhost
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: rustkit-viewhost*

