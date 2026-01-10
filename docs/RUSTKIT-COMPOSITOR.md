# RustKit Compositor

The compositor manages GPU rendering with per-view swapchain support, solving the WinCairo multi-view and resize issues.

## Overview

The compositor ensures:

1. **Per-view surfaces**: Each ViewId has its own wgpu Surface
2. **Resize correctness**: `resize_surface()` reconfigures swapchain immediately
3. **Multi-view rendering**: No global surface state
4. **Modern GPU support**: DirectX 12 or Vulkan backends

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Compositor                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  instance: wgpu::Instance                            │    │
│  │  adapter: wgpu::Adapter                              │    │
│  │  device: Arc<wgpu::Device>                           │    │
│  │  queue: Arc<wgpu::Queue>                             │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │       surfaces: HashMap<ViewId, SurfaceState>        │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         ▼                    ▼                    ▼
    ┌─────────┐          ┌─────────┐          ┌─────────┐
    │Surface 1│          │Surface 2│          │Surface 3│
    │(Swapchain)         │(Swapchain)         │(Swapchain)
    │ Chrome  │          │ Content │          │ Shelf   │
    └─────────┘          └─────────┘          └─────────┘
```

## API Reference

### Compositor

```rust
use rustkit_compositor::{Compositor, CompositorConfig};

// Create with defaults
let compositor = Compositor::new()?;

// Or with custom config
let config = CompositorConfig {
    vsync: true,
    format: wgpu::TextureFormat::Bgra8UnormSrgb,
    power_preference: wgpu::PowerPreference::HighPerformance,
};
let compositor = Compositor::with_config(config)?;
```

### Surface Management

```rust
// Create surface for a view's HWND
unsafe {
    compositor.create_surface_for_hwnd(view_id, hwnd, 800, 600)?;
}

// Resize on WM_SIZE
compositor.resize_surface(view_id, new_width, new_height)?;

// Or from ViewHost Bounds
compositor.resize_surface_from_bounds(view_id, bounds)?;

// Cleanup
compositor.destroy_surface(view_id)?;
```

### Rendering

```rust
// Basic solid color render (for testing)
compositor.render_solid_color(view_id, [0.1, 0.2, 0.3, 1.0])?;

// For actual content, get the surface texture and render
let surfaces = compositor.surfaces.read().unwrap();
let state = surfaces.get(&view_id).unwrap();
let texture = state.get_current_texture()?;

// Use texture.texture for render pass...
```

## Integration with ViewHost

```rust
use rustkit_viewhost::{ViewHost, ViewEvent, Bounds};
use rustkit_compositor::Compositor;

let mut host = ViewHost::new();
let compositor = Compositor::new()?;

// Create view
let bounds = Bounds::new(0, 0, 800, 600);
let view_id = host.create_view(parent_hwnd, bounds)?;
let hwnd = host.get_hwnd(view_id)?;

// Create surface for the view
unsafe {
    compositor.create_surface_for_hwnd(view_id, hwnd, 800, 600)?;
}

// Handle resize events
host.set_event_callback(Box::new(move |event| {
    if let ViewEvent::Resized { view_id, bounds, .. } = event {
        let _ = compositor.resize_surface_from_bounds(view_id, bounds);
    }
}));
```

## Solving WinCairo Issues

### Issue 1: Resize Not Updating Content

**WinCairo problem**: Compositor surface fixed at creation size.

**Solution**:
- `resize_surface()` calls `surface.configure()` with new dimensions
- Swapchain textures are recreated at correct size
- Next `get_current_texture()` returns properly sized buffer

### Issue 2: Multiple Views Fail

**WinCairo problem**: Global surface state.

**Solution**:
- `surfaces: HashMap<ViewId, SurfaceState>` isolates per-view
- Each view has its own swapchain
- Rendering one view doesn't affect others

## Performance Considerations

1. **Swapchain recreation**: Minimize by batching resize operations
2. **Present mode**: AutoVsync for smooth, AutoNoVsync for lower latency
3. **Texture format**: Bgra8UnormSrgb for best Windows compatibility
4. **Frame latency**: `desired_maximum_frame_latency: 2` balances smoothness and latency

## GPU Backend Selection

Priority order:
1. DirectX 12 (Windows native)
2. Vulkan (fallback)

```rust
let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
    ..Default::default()
});
```

## Error Handling

```rust
use rustkit_compositor::CompositorError;

match compositor.create_surface_for_hwnd(view_id, hwnd, 800, 600) {
    Ok(()) => { /* success */ }
    Err(CompositorError::DeviceCreation(msg)) => { /* GPU init failed */ }
    Err(CompositorError::SurfaceCreation(msg)) => { /* HWND issue */ }
    Err(e) => { /* other */ }
}
```

## Testing

```bash
# Unit tests (no GPU required)
cargo test -p rustkit-compositor

# Integration tests (requires display)
cargo test -p rustkit-compositor --features integration-tests
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: rustkit-compositor*

