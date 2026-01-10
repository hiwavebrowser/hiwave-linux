# RustKit Engine Roadmap

## Overview

RustKit is a Rust-native browser engine designed to replace WebKit's WinCairo port for the HiWave browser. This roadmap outlines the development phases beyond the initial MVP implementation.

**Current Status:** Phase 30 Complete (Accessibility) - Initial Roadmap Complete! 🎉
**Branch:** `master`

---

## Completed Phases (1-25)

| Phase | Name | Status | Description |
|-------|------|--------|-------------|
| 0 | Requirements | ✅ Complete | MVP requirements and acceptance tests |
| 1 | Fork & Harness | ✅ Complete | WebKit fork setup and embedder harness |
| 2 | Engine Skeleton | ✅ Complete | ViewHost, Compositor, Core runtime |
| 3 | HTML/DOM/CSS/Layout | ✅ Complete | DOM pipeline, CSS parsing, layout engine |
| 4 | JavaScript | ✅ Complete | Boa engine integration and DOM bindings |
| 5 | Networking | ✅ Complete | HTTP client, interception, downloads |
| 6 | Multi-view | ✅ Complete | Engine orchestration for multiple views |
| 7 | Observability | ✅ Complete | Logging, error handling, retry logic |
| 8 | HiWave Integration | ✅ Complete | IWebView adapter and feature flags |
| 9 | WPT Testing | ✅ Complete | Test harness for conformance testing |
| 10 | Benchmarks | ✅ Complete | Performance measurement infrastructure |
| 11 | WinCairo Removal | ✅ Complete | Removed legacy WebKit dependencies |
| 12 | CSS Box Model | ✅ Complete | Margin collapse, positioning, floats, z-index |
| 13 | Text Rendering | ✅ Complete | Font fallback, shaping, decorations |
| 14 | Event Handling | ✅ Complete | Mouse, keyboard, focus, touch, pointer events |
| 15 | Forms & Input | ✅ Complete | Text inputs, buttons, checkboxes, validation |
| 16 | Images & Media | ✅ Complete | PNG/JPEG/GIF/WebP decoding, background images |
| 17 | CSS Flexbox | ✅ Complete | Full flexbox layout implementation |
| 18 | Scrolling & Overflow | ✅ Complete | Scroll containers, smooth scroll, sticky |
| 19 | Navigation & History | ✅ Complete | History API, page lifecycle, hash navigation |
| 20 | Security & Isolation | ✅ Complete | CSP, CORS, same-origin, cookie security |
| 21 | CSS Grid Layout | ✅ Complete | Grid template, auto-placement, track sizing |
| 22 | Animations | ✅ Complete | CSS transitions, animations, timing functions |
| 23 | SVG Support | ✅ Complete | SVG parsing, paths, shapes, transforms |
| 24 | Canvas 2D | ✅ Complete | CanvasRenderingContext2D, paths, gradients |
| 25 | WebGL | ✅ Complete | WebGL 1.0 context, shaders, textures |
| 25.5 | GPU Renderer | ✅ Complete | Display list execution, text, images |
| 26 | Audio/Video | ✅ Complete | HTMLMediaElement, audio/video players |
| 27 | Service Workers | ✅ Complete | Registration, lifecycle, Cache API |
| 28 | IndexedDB | ✅ Complete | IDBFactory, object stores, transactions |
| 29 | Web Workers | ✅ Complete | DedicatedWorker, SharedWorker, MessageChannel |
| 30 | Accessibility | ✅ Complete | ARIA, accessibility tree, focus management |

---

## Phase Details (Recently Completed)

### Phase 14: Event Handling ✅
**Status:** Complete

Full event system implementation:

- [x] **Mouse events** - click, dblclick, mousedown/up/move, hover
- [x] **Keyboard events** - keydown, keyup, keypress, input
- [x] **Focus management** - Tab navigation, focus/blur, :focus-visible
- [x] **Hit testing** - Accurate element hit detection
- [x] **Event bubbling** - Capture and bubble phases with stopPropagation
- [x] **Pointer events** - Modern unified input API
- [x] **Touch events** - Multi-touch support
- [x] **Drag events** - DnD API with DataTransfer
- [x] **requestAnimationFrame** - 60fps animation scheduling

---

### Phase 15: Forms & Input ✅
**Status:** Complete

HTML form controls:

- [x] **Text inputs** - `<input type="text">`, `<textarea>`
- [x] **Buttons** - `<button>`, `<input type="submit/button">`
- [x] **Checkboxes & radios** - State management
- [x] **Form submission** - GET/POST with encoding
- [x] **Validation** - HTML5 constraint validation
- [x] **Caret rendering** - Blinking cursor with selection

---

### Phase 16: Images & Media ✅
**Status:** Complete

Visual media support:

- [x] **Image decoding** - PNG, JPEG, GIF, WebP (via `image` crate)
- [x] **Image sizing** - `width`, `height`, `object-fit`, `object-position`
- [x] **Background images** - `background-image`, `background-size`, `background-repeat`
- [x] **Lazy loading** - `loading="lazy"` support
- [x] **Responsive images** - `srcset`, `sizes` parsing
- [x] **Image caching** - Memory and disk cache

---

### Phase 17: CSS Flexbox ✅
**Status:** Complete

Full Flexbox layout:

- [x] **Flex container** - `display: flex/inline-flex`, direction, wrap
- [x] **Flex items** - `flex-grow`, `flex-shrink`, `flex-basis`
- [x] **Alignment** - `justify-content`, `align-items`, `align-content`, `align-self`
- [x] **Order** - `order` property
- [x] **Gap** - `gap`, `row-gap`, `column-gap`

---

### Phase 18: Scrolling & Overflow ✅
**Status:** Complete

Scrolling implementation:

- [x] **Overflow handling** - `overflow: scroll/auto/hidden/clip`
- [x] **Scroll containers** - Proper scrolling regions
- [x] **Smooth scroll** - `scroll-behavior: smooth`
- [x] **Wheel events** - Mouse wheel scrolling
- [x] **Scroll APIs** - `scrollTo`, `scrollBy`, `scrollIntoView`
- [x] **Sticky positioning** - `position: sticky`
- [x] **Scrollbar rendering** - Custom scrollbar support

---

### Phase 19: Navigation & History ✅
**Status:** Complete

Navigation system:

- [x] **History API** - `pushState`, `replaceState`, `popstate`
- [x] **Back/Forward** - Browser navigation
- [x] **Hash navigation** - `#fragment` scrolling
- [x] **Page lifecycle** - `DOMContentLoaded`, `load`, `beforeunload`, `unload`
- [x] **Navigation timing** - Performance metrics
- [x] **Location object** - Full location API

---

### Phase 20: Security & Isolation ✅
**Status:** Complete

Security implementation:

- [x] **Content Security Policy** - CSP header parsing & enforcement
- [x] **Same-origin policy** - Cross-origin restrictions
- [x] **CORS** - Preflight and response headers
- [x] **Secure contexts** - HTTPS detection
- [x] **Cookie security** - SameSite, Secure attributes
- [x] **Referrer policy** - Full referrer policy support
- [x] **Mixed content** - Block insecure content on secure pages

---

### Phase 21: CSS Grid Layout ✅
**Status:** Complete

Grid layout implementation:

- [x] **Grid container** - `display: grid/inline-grid`
- [x] **Grid template** - `grid-template-columns`, `grid-template-rows`
- [x] **Track sizing** - `fr` units, `minmax()`, `auto`
- [x] **Grid areas** - `grid-template-areas`, named areas
- [x] **Auto-placement** - Row/column major, dense packing
- [x] **Gap** - `row-gap`, `column-gap`
- [x] **Alignment** - `justify-items`, `align-items`, `justify-self`, `align-self`

---

### Phase 22: CSS Animations & Transitions ✅
**Status:** Complete

**New crate: `rustkit-animation`**

- [x] **CSS transitions** - `transition-property`, `duration`, `timing-function`, `delay`
- [x] **CSS animations** - `@keyframes`, `animation-*` properties
- [x] **Timing functions** - `ease`, `linear`, `cubic-bezier()`, `steps()`
- [x] **Animation events** - `animationstart`, `animationend`, `transitionend`
- [x] **Property interpolation** - Colors, lengths, transforms
- [x] **Animation timeline** - Central management of all animations

---

### Phase 23: SVG Support ✅
**Status:** Complete

**New crate: `rustkit-svg`**

- [x] **SVG parsing** - Document, viewBox, elements
- [x] **Basic shapes** - rect, circle, ellipse, line, polyline, polygon
- [x] **Paths** - Full SVG path commands (M, L, C, S, Q, T, A, Z)
- [x] **Styling** - fill, stroke, opacity, transforms
- [x] **Transforms** - translate, scale, rotate, matrix
- [x] **Text** - Basic SVG text rendering

---

### Phase 24: Canvas 2D ✅
**Status:** Complete

**New crate: `rustkit-canvas`**

- [x] **CanvasRenderingContext2D** - Full 2D context
- [x] **State management** - save(), restore(), transforms
- [x] **Paths** - moveTo, lineTo, arc, bezier curves
- [x] **Drawing** - fill, stroke, fillRect, strokeRect, clearRect
- [x] **Text** - fillText, strokeText, measureText
- [x] **Images** - drawImage with all overloads
- [x] **Gradients** - LinearGradient, RadialGradient
- [x] **Pixel manipulation** - getImageData, putImageData

---

### Phase 25: WebGL ✅
**Status:** Complete

**New crate: `rustkit-webgl`**

- [x] **WebGLRenderingContext** - WebGL 1.0 context
- [x] **Shaders** - Compile and link GLSL shaders
- [x] **Buffers** - Vertex and index buffers
- [x] **Textures** - 2D textures, texture parameters
- [x] **Framebuffers** - Off-screen rendering targets
- [x] **Drawing** - drawArrays, drawElements
- [x] **Uniforms** - All uniform types
- [x] **State** - Enable/disable, blend, depth, stencil

---

## Upcoming Phases (26-30)

### Phase 26: Audio/Video ✅
**Status:** Complete

Media elements:

- [x] **`<audio>` element** - Audio playback with controls
- [x] **`<video>` element** - Video playback with controls
- [x] **Media controls** - Play, pause, seek, volume
- [x] **Media events** - play, pause, ended, timeupdate
- [x] **HTMLMediaElement API** - currentTime, duration, volume
- [x] **Subtitles** - TextTrack and TextTrackCue support
- [ ] **Fullscreen** - Fullscreen API for video (TODO)

---

### Phase 27: Service Workers ✅
**Status:** Complete

- [x] **Registration** - `navigator.serviceWorker.register()`
- [x] **Lifecycle** - install, activate, fetch events
- [x] **Fetch interception** - Cache-first offline support
- [x] **Cache API** - `caches.open()`, `cache.add()`, `cache.match()`
- [x] **Clients API** - `clients.matchAll()`, `clients.openWindow()`, `clients.claim()`
- [ ] **Push notifications** - Push API (TODO)

---

### Phase 28: IndexedDB ✅
**Status:** Complete

- [x] **Database creation** - `indexedDB.open()`
- [x] **Object stores** - createObjectStore, add, get, put, delete
- [x] **Transactions** - readonly, readwrite, versionchange
- [x] **Indexes** - createIndex, unique, multi-entry
- [x] **Cursors** - openCursor, continue, advance
- [x] **Key paths** - single, compound, auto-increment

---

### Phase 29: Web Workers ✅
**Status:** Complete

- [x] **Dedicated Workers** - `new Worker(url)`
- [x] **Shared Workers** - `new SharedWorker(url)`
- [x] **Worker messaging** - postMessage, MessageChannel
- [x] **Transferable objects** - ArrayBuffer transfer
- [x] **Worker termination** - terminate()
- [x] **WorkerGlobalScope** - self, navigator, importScripts

---

### Phase 30: Accessibility ✅
**Status:** Complete

- [x] **ARIA roles** - All landmark, widget, document, table roles
- [x] **ARIA states** - Checked, Disabled, Expanded, Hidden, etc.
- [x] **Accessibility tree** - Parallel tree with DOM mapping
- [x] **Screen reader support** - Windows UIA dependencies
- [x] **Focus management** - Tab order, focus traps, history
- [x] **Live regions** - Polite/assertive announcements

---

## Future Considerations (31+)

| Phase | Feature | Description |
|-------|---------|-------------|
| 31 | WebRTC | RTCPeerConnection, MediaStream, data channels |
| 32 | Web Crypto | SubtleCrypto, key generation, encryption |
| 33 | WebSockets | Full-duplex communication |
| 34 | WebAssembly | WASM execution (via wasmer/wasmtime) |
| 35 | CSS Filters | blur, brightness, contrast, etc. |
| 36 | CSS Transforms 3D | perspective, rotateX/Y/Z |
| 37 | Shadow DOM | Component encapsulation |
| 38 | Custom Elements | Web Components v1 |
| 39 | Clipboard API | async clipboard access |
| 40 | Gamepad API | Controller input |

---

## Performance Milestones

| Milestone | Target | Current |
|-----------|--------|---------|
| DOM parse 100KB | < 10ms | TBD |
| Layout 1000 boxes | < 5ms | TBD |
| First paint | < 100ms | TBD |
| Time to interactive | < 500ms | TBD |
| Memory per tab | < 50MB | TBD |

---

## Crate Structure

```
# Core Engine
rustkit-common     # Shared utilities, logging, retry
rustkit-viewhost   # Win32 window hosting
rustkit-compositor # GPU rendering coordination
rustkit-renderer   # GPU display list execution
rustkit-core       # Task scheduling, navigation, history
rustkit-engine     # Orchestration, multi-view

# Independence Crates (replaced external deps)
rustkit-html       # HTML5 tokenizer & tree builder (replaced html5ever)
rustkit-cssparser  # CSS tokenizer & values (replaced cssparser)
rustkit-text       # DirectWrite text shaping (replaced dwrote)
rustkit-http       # HTTP client (replaced reqwest)
rustkit-codecs     # Image decoders (replaced image crate)

# Web Platform
rustkit-dom        # DOM tree, events, forms, images
rustkit-css        # CSS parsing, style computation
rustkit-layout     # Layout algorithms (block, flex, grid)
rustkit-js         # JavaScript engine (Boa)
rustkit-bindings   # JS ↔ DOM bridge, Web APIs
rustkit-net        # Fetch, downloads, security
rustkit-image      # Image loading and caching
rustkit-animation  # CSS animations and transitions
rustkit-svg        # SVG parsing and rendering
rustkit-canvas     # Canvas 2D API
rustkit-webgl      # WebGL API
rustkit-media      # Audio/video playback
rustkit-sw         # Service workers
rustkit-idb        # IndexedDB
rustkit-worker     # Web Workers
rustkit-a11y       # Accessibility

# Testing & Benchmarks
rustkit-test       # WPT harness
rustkit-bench      # Benchmarks
```

---

## Key Dependencies

### RustKit-Owned (Independence Project)

| Crate | Replaced | Purpose |
|-------|----------|---------|
| `rustkit-html` | `html5ever` | HTML5 tokenizer & tree builder |
| `rustkit-cssparser` | `cssparser` | CSS tokenizer & value parser |
| `rustkit-text` | `dwrote` | DirectWrite text shaping |
| `rustkit-http` | `reqwest` | HTTP/1.1 client with native-tls |
| `rustkit-codecs` | `image` | PNG/JPEG/GIF/WebP decoders |

### External Dependencies

| Crate | Purpose |
|-------|---------|
| `selectors` | CSS selector matching |
| `boa_engine` | JavaScript execution |
| `wgpu` | GPU rendering ( Phase Bravo 5 planned) |
| `tokio` | Async runtime |
| `native-tls` | TLS/SSL for HTTPS |

---

## Threading Model

- **Main thread**: Event loop, Win32 messages
- **Layout thread**: Style/layout computation (planned)
- **Script thread**: JavaScript execution
- **Network pool**: Async HTTP requests
- **Compositor thread**: GPU operations (planned)

---

## Test Coverage

| Crate | Tests |
|-------|-------|
| rustkit-core | 27 |
| rustkit-dom | 48 |
| rustkit-html | 86 |
| rustkit-css | 5 |
| rustkit-cssparser | 8 |
| rustkit-layout | 61 |
| rustkit-js | 11 |
| rustkit-bindings | 20 |
| rustkit-net | (varies) |
| rustkit-http | 12 |
| rustkit-image | 17 |
| rustkit-codecs | 15 |
| rustkit-text | 6 |
| rustkit-animation | 12 |
| rustkit-svg | 12 |
| rustkit-canvas | 12 |
| rustkit-webgl | 10 |
| rustkit-common | 13 |
| rustkit-engine | 7 |
| rustkit-bench | 3 |
| rustkit-test | (harness) |
| **Total** | **350+** |

---

## Contributing

1. Pick a phase/task from the roadmap
2. Create a branch: `rustkit-phase-N-feature`
3. Implement with tests
4. Run `cargo clippy --workspace` (no warnings)
5. Run `cargo test --workspace`
6. Submit PR with acceptance test evidence

---

## Resources

- [WPT Tests](https://github.com/nicotordev/nicot.web-platform-tests)
- [CSS Specs](https://www.w3.org/Style/CSS/)
- [HTML Spec](https://html.spec.whatwg.org/)
- [DOM Spec](https://dom.spec.whatwg.org/)
- [Canvas 2D Spec](https://html.spec.whatwg.org/multipage/canvas.html)
- [WebGL Spec](https://www.khronos.org/registry/webgl/specs/latest/1.0/)
- [Servo Browser Engine](https://servo.org/) (architecture reference)
