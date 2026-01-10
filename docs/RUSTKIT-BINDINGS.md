# RustKit Bindings

JavaScript-to-DOM bindings for the RustKit browser engine.

## Overview

RustKit Bindings provides:
- **Web API compatibility**: window, document, navigator, localStorage
- **DOM bridge**: JavaScript can query and manipulate the Rust DOM
- **Event system**: addEventListener/dispatchEvent support
- **Location API**: URL navigation and history

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      DomBindings                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  JsRuntime (rustkit-js)                              │    │
│  │  - Script evaluation                                 │    │
│  │  - Global objects                                    │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  WindowState                                         │    │
│  │  - location, dimensions                              │    │
│  │  - document reference                                │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Event Listeners                                     │    │
│  │  - Node → callback mapping                           │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
    ┌─────────┐          ┌─────────┐          ┌─────────┐
    │ window  │          │document │          │ events  │
    └─────────┘          └─────────┘          └─────────┘
```

## Usage

### Creating Bindings

```rust
use rustkit_js::JsRuntime;
use rustkit_bindings::DomBindings;

let runtime = JsRuntime::new()?;
let bindings = DomBindings::new(runtime)?;

// Window and document are now available in JS
bindings.evaluate("console.log(window.navigator.userAgent)")?;
```

### Binding a Document

```rust
use rustkit_dom::Document;
use rustkit_bindings::DomBindings;

let html = "<html><head><title>Test</title></head><body></body></html>";
let doc = Rc::new(Document::parse_html(html)?);

let bindings = DomBindings::new(JsRuntime::new()?)?;
bindings.set_document(doc)?;

// Now document.title works
let title = bindings.evaluate("document.title")?;
```

### Setting Location

```rust
use url::Url;
use rustkit_bindings::DomBindings;

let bindings = DomBindings::new(JsRuntime::new()?)?;
let url = Url::parse("https://example.com/page?q=1#section")?;
bindings.set_location(&url)?;

// Location is now accessible
bindings.evaluate("console.log(window.location.href)")?;
```

### Window Dimensions

```rust
use rustkit_bindings::DomBindings;

let bindings = DomBindings::new(JsRuntime::new()?)?;
bindings.set_dimensions(1920.0, 1080.0)?;

// Scripts can read dimensions
bindings.evaluate("console.log(window.innerWidth, window.innerHeight)")?;
```

## Available APIs

### Window Object

| Property/Method | Description |
|-----------------|-------------|
| `window.innerWidth` | Viewport width |
| `window.innerHeight` | Viewport height |
| `window.outerWidth` | Window width |
| `window.outerHeight` | Window height |
| `window.devicePixelRatio` | Display scaling factor |
| `window.location` | Location object |
| `window.navigator` | Navigator object |
| `window.history` | History object |
| `window.localStorage` | Local storage |
| `window.sessionStorage` | Session storage |
| `window.alert()` | Show alert (logged) |
| `window.confirm()` | Show confirm (returns false) |
| `window.prompt()` | Show prompt (returns default) |

### Location Object

| Property/Method | Description |
|-----------------|-------------|
| `location.href` | Full URL |
| `location.protocol` | Protocol (e.g., "https:") |
| `location.host` | Host with port |
| `location.hostname` | Host without port |
| `location.port` | Port number |
| `location.pathname` | Path |
| `location.search` | Query string |
| `location.hash` | Fragment |
| `location.origin` | Origin |
| `location.reload()` | Reload page |
| `location.replace()` | Replace URL |
| `location.assign()` | Navigate to URL |

### Navigator Object

| Property | Value |
|----------|-------|
| `navigator.userAgent` | "RustKit/1.0" |
| `navigator.language` | "en-US" |
| `navigator.languages` | ["en-US", "en"] |
| `navigator.platform` | "Win32" |
| `navigator.onLine` | true |

### Document Object

| Property/Method | Description |
|-----------------|-------------|
| `document.title` | Page title |
| `document.readyState` | Loading state |
| `document.URL` | Current URL |
| `document.getElementById()` | Find by ID |
| `document.getElementsByTagName()` | Find by tag |
| `document.getElementsByClassName()` | Find by class |
| `document.querySelector()` | CSS selector |
| `document.querySelectorAll()` | CSS selector (all) |
| `document.createElement()` | Create element |
| `document.createTextNode()` | Create text node |

### Storage APIs

```javascript
// localStorage
localStorage.setItem('key', 'value');
var value = localStorage.getItem('key');
localStorage.removeItem('key');
localStorage.clear();

// sessionStorage (same API)
sessionStorage.setItem('key', 'value');
```

## Event System

### Adding Listeners

```rust
use rustkit_bindings::DomBindings;
use rustkit_dom::NodeId;

let bindings = DomBindings::new(JsRuntime::new()?)?;

// Add a click listener
let listener_id = bindings.add_event_listener(
    NodeId::new(1),
    "click",
    "console.log('clicked!')",
    false
);

// Remove when done
bindings.remove_event_listener(listener_id);
```

### Dispatching Events

```rust
use rustkit_bindings::DomBindings;
use rustkit_dom::NodeId;

let bindings = DomBindings::new(JsRuntime::new()?)?;

// Dispatch a click event
bindings.dispatch_event(NodeId::new(1), "click")?;
```

## Integration Example

```rust
use rustkit_dom::Document;
use rustkit_js::JsRuntime;
use rustkit_bindings::DomBindings;
use url::Url;
use std::rc::Rc;

// Parse HTML
let html = r#"
<!DOCTYPE html>
<html>
<head><title>My Page</title></head>
<body>
    <div id="app">Loading...</div>
    <script>
        document.getElementById('app').textContent = 'Hello, World!';
    </script>
</body>
</html>
"#;

let doc = Rc::new(Document::parse_html(html)?);

// Create bindings
let runtime = JsRuntime::new()?;
let bindings = DomBindings::new(runtime)?;

// Bind document and location
bindings.set_document(doc)?;
bindings.set_location(&Url::parse("https://example.com/")?)?;

// Execute inline scripts
bindings.evaluate("document.getElementById('app').textContent = 'Hello!'")?;
```

## Testing

```bash
# Run bindings tests
cargo test -p rustkit-bindings

# With logging
RUST_LOG=rustkit_bindings=debug cargo test -p rustkit-bindings
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: dom-bindings*

