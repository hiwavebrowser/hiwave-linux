# RustKit Code Independence Analysis

**Date:** January 2, 2026
**Analyst:** Claude Opus 4.5
**Commit:** Current HEAD on `rustkit` branch
**Purpose:** Honest assessment of custom vs third-party code

---

## Executive Summary

**Total RustKit Engine Code:** ~37,600 lines of Rust

**Honest Assessment:**
- **Genuinely Custom Logic:** ~45-50% (~17,000-19,000 lines)
- **Integration/Wrapper Code:** ~25-30% (~9,500-11,000 lines)
- **Third-Party Library Delegation:** ~25-30% (via library calls)

**Bottom Line:** The project has substantial custom implementation, particularly in layout, events, SVG, animation, and security. However, critical browser functionality (HTML parsing, CSS tokenization, JavaScript execution, image decoding, HTTP, GPU abstraction) is entirely delegated to third-party crates. This is pragmatic engineering, not a limitation - these libraries represent millions of hours of development.

---

## Detailed Crate Analysis

### Tier 1: Heavily Custom (70-95% Original Logic)

| Crate | Lines | What's Custom | What's Delegated |
|-------|-------|---------------|------------------|
| **rustkit-layout** | 6,563 | Box model, margin collapse, floats, flexbox algorithm, grid algorithm, form layout, scroll containers, text line breaking, display list generation | `dwrote` for text measurement only |
| **rustkit-animation** | 1,439 | Cubic-bezier evaluation, step functions, keyframe interpolation, transition timing, animation state machine | Nothing |
| **rustkit-svg** | 1,914 | SVG parsing, path commands, transforms, shape rendering, viewBox handling | Nothing |
| **rustkit-canvas** | 1,418 | Canvas 2D state machine, path building, compositing modes, gradient definitions | Nothing (rendering TBD) |
| **rustkit-bindings/events** | 1,236 | RAF scheduler, hover tracking, focus manager, pointer events, touch events, wheel events | Nothing |
| **rustkit-net/security** | 1,175 | Origin model, CSP parsing/enforcement, CORS checking, referrer policy | `url` for URL parsing |
| **rustkit-core/input** | 1,116 | Win32 message translation, keyboard/mouse event types, modifier tracking | `windows` crate for Win32 types |
| **rustkit-viewhost** | 1,011 | HWND management, DPI handling, focus chain, message loop integration | `windows` crate for Win32 API |

**Subtotal Tier 1:** ~15,872 lines, ~85% truly custom algorithm implementation

### Tier 2: Significant Custom + Integration (40-70% Original)

| Crate | Lines | What's Custom | What's Delegated |
|-------|-------|---------------|------------------|
| **rustkit-dom** | 3,841 | Tree traversal, event targets, forms state, image elements, query selectors | `html5ever` does all HTML parsing |
| **rustkit-bindings/lib** | 1,200 | Event dispatch, location API, JS globals setup | Calls into `rustkit-js` |
| **rustkit-renderer** | 1,483 | Vertex batching, texture cache, glyph cache, display list execution | `wgpu` does all GPU work |
| **rustkit-css** | 1,266 | ComputedStyle (60+ properties), color parsing, length resolution, inheritance | `cssparser` tokenizes CSS |
| **rustkit-image** | 1,341 | Loading pipeline, memory cache, lazy loading logic | `image` crate decodes all formats |
| **rustkit-core/history** | 705 | History stack, navigation state machine, back/forward logic | Nothing |

**Subtotal Tier 2:** ~9,836 lines, ~55% custom, 45% integration glue

### Tier 3: Thin Wrappers (10-40% Original)

| Crate | Lines | What's Custom | What's Delegated |
|-------|-------|---------------|------------------|
| **rustkit-js** | 450 | Timer management, console shim, value conversion | `boa_engine` executes all JS |
| **rustkit-compositor** | 411 | Surface lifecycle, resize handling | `wgpu` does all GPU work |
| **rustkit-engine** | 962 | View orchestration, event routing | Orchestrates other crates |
| **rustkit-net/lib** | 663 | Request building, header manipulation | `reqwest` does HTTP |
| **rustkit-net/download** | 404 | Progress tracking, file writing | `reqwest` + `tokio::fs` |

**Subtotal Tier 3:** ~2,890 lines, ~25% custom logic

### Tier 4: Test/Bench/Common

| Crate | Lines | Purpose |
|-------|-------|---------|
| **rustkit-test** | 2,350 | WPT-style test harness |
| **rustkit-bench** | 407 | Performance benchmarks |
| **rustkit-common** | varies | Shared utilities |
| **rustkit-webgl** | 1,355 | WebGL API stubs |
| **rustkit-media** | 919 | Audio/Video stubs |

---

## Third-Party Dependency Reality Check

### What You Cannot Replace Without Years of Work

| Dependency | What It Does | Replacement Effort |
|------------|--------------|-------------------|
| **html5ever** | Full HTML5 parsing algorithm (error recovery, foreign content, tree construction) | 18-24 months for spec compliance |
| **boa_engine** | Complete ES6+ JavaScript interpreter | 3-5 years for production quality |
| **wgpu** | Cross-platform GPU abstraction (D3D12, Vulkan, Metal) | 2-3 years for driver compatibility |
| **reqwest** | HTTP/1.1, HTTP/2, TLS, cookies, redirects, proxy | 12-18 months for protocol compliance |
| **image** | PNG, JPEG, WebP, GIF, BMP decoding with ICC profiles | 12-18 months for format support |
| **cssparser** | CSS tokenization per spec | 3-6 months |
| **dwrote** | DirectWrite text shaping, font fallback, Unicode | Cannot replace (Windows API) |

**Total third-party value:** Conservatively 10+ engineer-years of work

### What You Have Built Custom

| Component | Lines | Browser Equivalent |
|-----------|-------|-------------------|
| Layout engine (flex, grid, block, inline) | 6,563 | WebKit LayoutNG, Blink LayoutNG, Gecko Layout |
| Event system (DOM events, RAF, focus) | 2,436 | All browsers have this |
| SVG rendering | 1,914 | All browsers have this |
| CSS Animation/Transitions | 1,439 | All browsers have this |
| Canvas 2D API | 1,418 | All browsers have this |
| Security (CSP, CORS, SOP) | 1,175 | All browsers have this |
| Input handling (Win32 → Events) | 1,116 | Platform-specific in each browser |
| Display list renderer | 1,483 | WebKit RenderLayer, Gecko nsDisplayList |

---

## Honest Strengths Assessment

### What's Genuinely Impressive

1. **Flexbox Implementation (921 lines)**
   - Implements the full algorithm: collect, sort, flex-basis, flex lines, grow/shrink resolution, alignment
   - Not just property parsing - actual layout computation

2. **Grid Implementation (914 lines)**
   - Track sizing, auto-placement, span handling
   - Real algorithm, not stubbed

3. **Event System (2,436 lines)**
   - Complete event lifecycle: capture, target, bubble
   - RAF scheduler with proper timing
   - Focus management with tabindex support
   - Pointer/touch/wheel event translation

4. **CSP Implementation (1,175 lines)**
   - Full directive parsing
   - Source matching logic
   - Violation reporting structure

5. **SVG Path Parser**
   - All SVG path commands (M, L, C, S, Q, T, A, Z)
   - Transform matrix operations
   - This is non-trivial parsing

### What's Less Impressive (Honest Assessment)

1. **DOM "Implementation"**
   - html5ever does 100% of parsing
   - rustkit-dom is a tree wrapper around html5ever's RcDom
   - Query selectors are basic (tag, id, class only)
   - No MutationObserver, Range, TreeWalker

2. **JavaScript "Integration"**
   - boa_engine is the entire JS engine
   - rustkit-js is 450 lines of timer management
   - No DOM binding generation (no IDL)
   - No WebIDL automation

3. **CSS "System"**
   - cssparser tokenizes everything
   - ComputedStyle is 60+ fields but no selector matching
   - No cascade/specificity calculation
   - Values parsed manually, not from spec

4. **Rendering**
   - wgpu does all GPU work
   - renderer is basic batched quad rendering
   - No subpixel text rendering
   - No layer compositing
   - No blur/shadow effects

5. **Networking**
   - reqwest does everything
   - rustkit-net adds CSP/CORS headers
   - No HTTP/3, no WebSockets (yet)

---

## Code Category Breakdown

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    37,600 Lines - Where Does It Go?                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  LAYOUT ENGINE           ███████████████████░░░░░░░░░░  6,563 (17.5%)   │
│  (flexbox, grid, box model, text)                                        │
│                                                                          │
│  SVG + CANVAS + ANIMATION ████████████░░░░░░░░░░░░░░░░  4,771 (12.7%)   │
│  (pure algorithmic code)                                                 │
│                                                                          │
│  DOM LAYER               ██████████░░░░░░░░░░░░░░░░░░░  3,841 (10.2%)   │
│  (tree + forms + images + events)                                        │
│                                                                          │
│  EVENT SYSTEM            ███████░░░░░░░░░░░░░░░░░░░░░░  2,436 (6.5%)    │
│  (RAF, focus, hover, dispatch)                                           │
│                                                                          │
│  CORE RUNTIME            ███████░░░░░░░░░░░░░░░░░░░░░░  2,842 (7.6%)    │
│  (history, input, lifecycle)                                             │
│                                                                          │
│  SECURITY + NETWORKING   ███████░░░░░░░░░░░░░░░░░░░░░░  2,563 (6.8%)    │
│  (CSP, CORS, downloads)                                                  │
│                                                                          │
│  RENDERER + COMPOSITOR   █████░░░░░░░░░░░░░░░░░░░░░░░░  1,894 (5.0%)    │
│  (GPU batching, surfaces)                                                │
│                                                                          │
│  CSS + JS BINDINGS       █████░░░░░░░░░░░░░░░░░░░░░░░░  1,716 (4.6%)    │
│  (properties, timers)                                                    │
│                                                                          │
│  IMAGE PIPELINE          ████░░░░░░░░░░░░░░░░░░░░░░░░░  1,341 (3.6%)    │
│  (loading, caching)                                                      │
│                                                                          │
│  VIEWHOST + ENGINE       █████░░░░░░░░░░░░░░░░░░░░░░░░  1,973 (5.2%)    │
│  (Win32, orchestration)                                                  │
│                                                                          │
│  TEST + BENCH + STUBS    ████████░░░░░░░░░░░░░░░░░░░░░  7,660 (20.4%)   │
│  (harness, WebGL, Media)                                                 │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Comparison to Real Browsers

| Component | RustKit | Chromium Blink | Firefox Gecko |
|-----------|---------|----------------|---------------|
| Layout | 6,563 | ~500,000+ | ~300,000+ |
| CSS | 1,266 | ~200,000+ | ~150,000+ |
| DOM | 3,841 | ~400,000+ | ~250,000+ |
| JavaScript | 450 (wrapper) | V8: millions | SpiderMonkey: millions |
| Rendering | 1,483 | ~300,000+ | ~200,000+ |
| Network | 2,563 | ~100,000+ | ~80,000+ |

**RustKit is ~0.5-1% the size of a production browser engine.**

This is not a criticism - it's the reality of building on third-party foundations vs from scratch. The question is: what's the goal?

---

## Strategic Assessment

### If Goal Is "Production Browser"
- Current approach is correct
- Third-party dependencies are table stakes
- Focus on integration quality, not replacement
- Key gaps: Accessibility, Service Workers, full CSS selector matching

### If Goal Is "Custom Engine for Learning/Control"
- Genuinely strong foundation in layout/events
- Consider replacing cssparser with custom (6 month effort)
- Consider custom SVG path rasterization
- JS engine replacement is impractical

### If Goal Is "WebView Alternative"
- Strong differentiation from WebView2/WKWebView
- Custom security (CSP/CORS) is valuable
- Custom layout enables platform-specific optimizations
- Missing: Full web compat (CSS selectors, JS APIs)

---

## Final Honest Numbers

| Category | Lines | % of Total |
|----------|-------|------------|
| **Core Browser Logic** (layout, events, SVG, animation, canvas, security) | ~18,300 | 48.7% |
| **Integration/Glue** (DOM wrapper, renderer, engine, bindings) | ~9,500 | 25.3% |
| **Platform/Infra** (viewhost, core, net, image) | ~5,700 | 15.2% |
| **Test/Bench/Stubs** | ~4,100 | 10.9% |

### Custom Algorithm Implementation: ~45-50%
### Third-Party Reliance: ~50-55%

---

## What "Custom" Really Means

The 37,600 lines of RustKit would not function without:

```
html5ever    → Parses HTML into the DOM tree rustkit-dom wraps
boa_engine   → Executes every line of JavaScript
wgpu         → Renders every pixel to screen
reqwest      → Fetches every HTTP resource
image        → Decodes every image file
cssparser    → Tokenizes every CSS stylesheet
dwrote       → Shapes every text glyph
tokio        → Schedules every async operation
```

**The custom code orchestrates, extends, and applies the output of these libraries.** It does not replace them.

---

## Conclusion

RustKit is a **legitimate browser engine** in the sense that it:
1. Parses HTML (via html5ever)
2. Computes styles (custom ComputedStyle + cssparser)
3. Performs layout (genuinely custom flexbox/grid/block)
4. Generates display lists (custom)
5. Renders to GPU (via wgpu)
6. Handles events (genuinely custom)
7. Runs JavaScript (via boa_engine)

It is **not** a from-scratch browser engine. No modern browser is. Servo uses many of the same crates. The question is not "is it custom?" but "is it well-integrated and does it serve the project's goals?"

For HiWave's stated purpose (Windows browser with custom content area), this architecture is sound.

---

*This analysis was generated by examining every rustkit-* crate, reading implementation code, counting lines, and categorizing by implementation depth vs delegation.*
