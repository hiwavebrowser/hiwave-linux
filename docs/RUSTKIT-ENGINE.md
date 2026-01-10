# RustKit Engine

The orchestration layer that integrates all RustKit components into a complete browser engine with multi-view support.

## Overview

RustKit Engine provides:
- **Multi-view management**: Create and manage multiple independent browser views
- **Unified API**: Single entry point for all browser functionality
- **Event coordination**: Route events between views and host application
- **Resource sharing**: Share compositor and network resources efficiently

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Engine                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Views: HashMap<EngineViewId, ViewState>             │    │
│  │  - document, layout, JS runtime                      │    │
│  │  - navigation state                                  │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Shared Resources                                    │    │
│  │  - ViewHost (window management)                      │    │
│  │  - Compositor (GPU rendering)                        │    │
│  │  - ResourceLoader (networking)                       │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Event Channel                                       │    │
│  │  - mpsc::UnboundedSender<EngineEvent>                │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
         │              │              │              │
         ▼              ▼              ▼              ▼
    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
    │  View 1 │    │  View 2 │    │  View 3 │    │  View N │
    │ (Chrome)│    │(Content)│    │ (Shelf) │    │  (...)  │
    └─────────┘    └─────────┘    └─────────┘    └─────────┘
```

## Usage

### Creating an Engine

```rust
use rustkit_engine::{Engine, EngineBuilder};

// Using builder
let engine = EngineBuilder::new()
    .user_agent("MyBrowser/1.0")
    .javascript_enabled(true)
    .cookies_enabled(true)
    .background_color([1.0, 1.0, 1.0, 1.0])
    .build()?;

// Or with default config
let engine = Engine::new(EngineConfig::default())?;
```

### Creating Views

```rust
use rustkit_engine::Engine;
use rustkit_viewhost::Bounds;

// Create multiple views (HiWave's three-view architecture)
let chrome_view = engine.create_view(parent_hwnd, Bounds::new(0, 0, 1920, 72))?;
let content_view = engine.create_view(parent_hwnd, Bounds::new(0, 72, 1920, 1000))?;
let shelf_view = engine.create_view(parent_hwnd, Bounds::new(0, 1072, 1920, 48))?;

println!("Created {} views", engine.view_count());
```

### Loading URLs

```rust
use url::Url;

// Load content
let url = Url::parse("https://example.com")?;
engine.load_url(content_view, url).await?;

// Get page info
if let Some(title) = engine.get_title(content_view) {
    println!("Page title: {}", title);
}
```

### Handling Events

```rust
use rustkit_engine::EngineEvent;

// Get event receiver
let mut events = engine.take_event_receiver().unwrap();

// Handle events
tokio::spawn(async move {
    while let Some(event) = events.recv().await {
        match event {
            EngineEvent::NavigationStarted { view_id, url } => {
                println!("Loading: {}", url);
            }
            EngineEvent::PageLoaded { view_id, url, title } => {
                println!("Loaded: {} - {:?}", url, title);
            }
            EngineEvent::TitleChanged { view_id, title } => {
                // Update window title
            }
            EngineEvent::ViewResized { view_id, width, height } => {
                // Handle resize
            }
            EngineEvent::NavigationFailed { view_id, url, error } => {
                eprintln!("Failed to load {}: {}", url, error);
            }
            _ => {}
        }
    }
});
```

### Resizing Views

```rust
use rustkit_viewhost::Bounds;

// Resize content view (e.g., when sidebar opens)
let new_bounds = Bounds::new(200, 72, 1720, 1000);
engine.resize_view(content_view, new_bounds)?;
```

### Executing JavaScript

```rust
// Execute script
let result = engine.execute_script(content_view, "document.title")?;
println!("Title: {}", result);

// Modify DOM
engine.execute_script(content_view, "document.body.style.background = 'red'")?;
```

## Engine Events

| Event | Description |
|-------|-------------|
| `NavigationStarted` | URL loading started |
| `NavigationCommitted` | First bytes received |
| `PageLoaded` | Page fully loaded |
| `NavigationFailed` | Loading failed |
| `TitleChanged` | Page title changed |
| `ConsoleMessage` | JavaScript console output |
| `ViewResized` | View dimensions changed |
| `ViewFocused` | View received focus |
| `DownloadStarted` | File download initiated |

## View State

Each view maintains:
- **URL**: Current page URL
- **Title**: Page title
- **Document**: Parsed DOM tree
- **Stylesheet**: Parsed CSS
- **Layout**: Computed layout tree
- **Display List**: Paint commands
- **JS Runtime**: JavaScript context
- **Navigation**: History and state

## Multi-View Patterns

### HiWave Three-View Architecture

```rust
// Chrome UI (static HTML)
let chrome = engine.create_view(hwnd, Bounds::new(0, 0, w, 72))?;
engine.load_url(chrome, "hiwave://chrome").await?;

// Main content (web pages)
let content = engine.create_view(hwnd, Bounds::new(0, 72, w, h - 120))?;
engine.load_url(content, "https://example.com").await?;

// Shelf (bookmarks, etc.)
let shelf = engine.create_view(hwnd, Bounds::new(0, h - 48, w, 48))?;
engine.load_url(shelf, "hiwave://shelf").await?;
```

### Resize Handling

```rust
fn apply_layout(engine: &mut Engine, chrome: EngineViewId, content: EngineViewId, shelf: EngineViewId, width: u32, height: u32, sidebar_width: u32) {
    // Chrome takes full width at top
    engine.resize_view(chrome, Bounds::new(0, 0, width, 72)).unwrap();
    
    // Content adjusts for sidebar
    let content_x = sidebar_width as i32;
    let content_w = width - sidebar_width;
    engine.resize_view(content, Bounds::new(content_x, 72, content_w, height - 120)).unwrap();
    
    // Shelf at bottom
    engine.resize_view(shelf, Bounds::new(content_x, height - 48, content_w, 48)).unwrap();
}
```

## Navigation

```rust
// Navigate
engine.load_url(view, Url::parse("https://example.com")?).await?;

// Check history
if engine.can_go_back(view) {
    // Show back button
}

if engine.can_go_forward(view) {
    // Show forward button
}
```

## Downloads

```rust
// Get download manager
let downloads = engine.download_manager();

// Handle download events
// (See rustkit-net documentation)
```

## Error Handling

```rust
use rustkit_engine::EngineError;

match engine.load_url(view, url).await {
    Ok(()) => { /* success */ }
    Err(EngineError::NetworkError(e)) => {
        eprintln!("Network error: {}", e);
    }
    Err(EngineError::NavigationError(e)) => {
        eprintln!("Navigation error: {}", e);
    }
    Err(EngineError::ViewNotFound(id)) => {
        eprintln!("View not found: {:?}", id);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Testing

```bash
# Run engine tests
cargo test -p rustkit-engine

# With logging
RUST_LOG=rustkit_engine=debug cargo test -p rustkit-engine
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: multiview-integration*

