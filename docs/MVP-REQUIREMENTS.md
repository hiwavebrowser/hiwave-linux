# Rust WebKit Rewrite: MVP Requirements Matrix

This document defines the minimum viable product (MVP) requirements for the HiWave Rust browser engine rewrite, derived from the limitations documented in [WINCAIRO-LIMITATIONS.md](./WINCAIRO-LIMITATIONS.md).

## Table of Contents

- [Target Sites](#target-sites)
- [Required Web APIs](#required-web-apis)
- [Performance Targets](#performance-targets)
- [Stability Requirements](#stability-requirements)
- [Success Criteria by Limitation](#success-criteria-by-limitation)
- [Acceptance Test Matrix](#acceptance-test-matrix)

---

## Target Sites

The MVP must be able to render and interact with these categories of sites:

### Tier 1: Must Work (Day 1)

| Site | Category | Critical Features |
|------|----------|-------------------|
| **HiWave Internal Pages** | Built-in | new-tab, settings, about, report pages |
| **Wikipedia** | Static content | Text rendering, images, internal links |
| **GitHub** (logged out) | Documentation | README rendering, code viewing, navigation |
| **DuckDuckGo** | Search | Form submission, results navigation |
| **Hacker News** | Static/minimal JS | Link navigation, basic CSS |

### Tier 2: Should Work (Week 2-4)

| Site | Category | Critical Features |
|------|----------|-------------------|
| **GitHub** (logged in) | OAuth + SPA | Login flow, issue navigation, basic interactions |
| **Google Search** | Dynamic | Search suggestions, results, SafeSearch |
| **Reddit** (old.reddit.com) | Classic web | Comments, voting (visual), navigation |
| **News sites** (BBC, NPR, Reuters) | Content | Articles, images, video embeds (placeholder OK) |
| **StackOverflow** | Documentation | Code blocks, syntax highlighting, answers |

### Tier 3: Nice to Have (Month 2+)

| Site | Category | Notes |
|------|----------|-------|
| **YouTube** | Video | Video playback can be deferred to post-MVP |
| **Twitter/X** | Heavy SPA | Complex JS; acceptable to fail gracefully |
| **Modern SPAs** | React/Vue/etc. | Progressive support |

---

## Required Web APIs

### Critical (MVP Blocker)

| API | Priority | Test Coverage |
|-----|----------|---------------|
| `document` / `window` | P0 | DOM manipulation tests |
| `fetch` / `XMLHttpRequest` | P0 | Network request tests |
| `localStorage` / `sessionStorage` | P0 | Persistence tests |
| `history` / `location` | P0 | Navigation tests |
| `setTimeout` / `setInterval` | P0 | Timer tests |
| `console.*` | P0 | Debug output capture |
| `JSON` | P0 | Parse/stringify tests |
| `Promise` / `async/await` | P0 | Async flow tests |
| Cookie handling | P0 | Set/get/expiry tests |
| `addEventListener` / events | P0 | Event dispatch tests |

### Important (First Month)

| API | Priority | Notes |
|-----|----------|-------|
| `FormData` | P1 | Form submission |
| `URLSearchParams` | P1 | Query string handling |
| `TextEncoder` / `TextDecoder` | P1 | String encoding |
| `Blob` / `File` | P1 | File handling basics |
| `URL` | P1 | URL parsing |
| CORS handling | P1 | Cross-origin requests |
| CSP (basic) | P1 | Security headers |
| `MutationObserver` | P1 | DOM change observation |
| `requestAnimationFrame` | P1 | Animation timing |

### Deferred (Post-MVP)

| API | Notes |
|-----|-------|
| `WebSocket` | Real-time features |
| `WebRTC` | Video/audio calls |
| `IndexedDB` | Complex storage |
| `WebGL` / `WebGPU` | 3D graphics |
| `ServiceWorker` | Offline/PWA |
| `Web Audio` | Audio processing |
| `WebAssembly` | WASM execution |

---

## Performance Targets

### Rendering Performance

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| **First Contentful Paint** | < 500ms | Internal page load |
| **Time to Interactive** | < 1000ms | Simple page with JS |
| **Resize Response** | < 16ms (60fps) | Rapid `set_bounds` calls |
| **Paint after resize** | < 32ms | Content matches new bounds |

### Memory Limits

| Metric | Target | Notes |
|--------|--------|-------|
| **Idle memory per view** | < 50MB | Empty page |
| **Peak memory per view** | < 200MB | Complex page |
| **Memory after close** | Return to baseline | No leaks |

### Responsiveness

| Metric | Target |
|--------|--------|
| **Input latency** | < 50ms |
| **Scroll smoothness** | 60fps |
| **View switch time** | < 100ms |

---

## Stability Requirements

### Crash-Free Operation

| Requirement | Target |
|-------------|--------|
| **1-hour stress test** | 0 crashes |
| **Rapid resize cycles** | 10,000 iterations, 0 failures |
| **View create/destroy** | 100 cycles, 0 leaks |
| **Navigation stress** | 500 navigations, 0 hangs |

### Deterministic Behavior

| Requirement | Verification |
|-------------|--------------|
| **Shutdown clean** | All resources released within 5s |
| **Event ordering** | start → commit → finish (always) |
| **Error recovery** | Graceful fallback on network errors |

---

## Success Criteria by Limitation

Based on [WINCAIRO-LIMITATIONS.md](./WINCAIRO-LIMITATIONS.md), each limitation has explicit pass/fail criteria:

### 1. View Resize Does Not Update Rendered Content

**BLOCKING → RESOLVED when:**

```
✓ HWND resize triggers compositor surface resize
✓ Content repaints within 32ms of `set_bounds` call
✓ Sidebar open/close shows correct content bounds
✓ Window resize shows correct content at all sizes
✓ No visual artifacts during rapid resize
```

**Acceptance Tests:**
- `test_resize_basic` - Single resize updates content
- `test_resize_rapid_10000` - 10,000 resize cycles without failure
- `test_resize_sidebar_toggle` - Sidebar open/close/drag
- `test_resize_pixel_verification` - Screenshot comparison pre/post resize

### 2. Multiple Concurrent WebViews Not Supported

**BLOCKING → RESOLVED when:**

```
✓ 3+ views can render simultaneously
✓ Each view has independent surface/swapchain
✓ Creating view N does not blank views 1..N-1
✓ Each view responds to its own resize events
✓ Destroying one view does not affect others
```

**Acceptance Tests:**
- `test_multiview_render_3` - 3 views render distinct content
- `test_multiview_create_destroy_100` - Create/destroy 100 views
- `test_multiview_independent_resize` - Each view resizes independently
- `test_multiview_concurrent_navigation` - Navigate views in parallel

### 3. Page Load Events Not Implemented

**RESOLVED when:**

```
✓ didStartProvisionalLoad fires on navigation start
✓ didCommitLoad fires when first byte received
✓ didFinishLoad fires when DOM fully loaded
✓ didFailLoad fires on network/HTTP error
✓ Progress updates (0-100%) during load
✓ is_loading() returns correct state
```

**Acceptance Tests:**
- `test_load_events_ordering` - Events fire in correct order
- `test_load_events_failure` - Error events on bad URL
- `test_load_events_redirect` - Events during redirect chain
- `test_load_progress` - Progress callbacks during load

### 4. Navigation Interception Limited

**RESOLVED when:**

```
✓ Request interception at network layer (before fetch)
✓ Resource type classification (document, script, image, etc.)
✓ Policy decision callback with allow/block/redirect
✓ HiWave Shield integration works without JS injection
✓ Response interception for headers/body modification
```

**Acceptance Tests:**
- `test_intercept_block_request` - Block request before network
- `test_intercept_resource_type` - Correct type classification
- `test_intercept_redirect` - Intercept and redirect
- `test_shield_integration` - Shield blocks ads/trackers

### 5. Download Handlers Not Implemented

**RESOLVED when:**

```
✓ Download detection (Content-Disposition, MIME type)
✓ Save-to-disk with user-specified path
✓ Progress reporting (bytes received / total)
✓ Cancel/pause/resume support
✓ Filename resolution (from header or URL)
✓ Integration with HiWave download UI
```

**Acceptance Tests:**
- `test_download_basic` - Download file to disk
- `test_download_progress` - Progress callbacks
- `test_download_cancel` - Cancel mid-download
- `test_download_resume` - Resume interrupted download

### 6. New Window/Popup Handling

**RESOLVED when:**

```
✓ createNewPage callback fires for target="_blank"
✓ Popup policy callback (allow/block/redirect)
✓ window.open() handled correctly
✓ Default popup blocking matches HiWave heuristics
```

**Acceptance Tests:**
- `test_popup_target_blank` - target="_blank" detected
- `test_popup_window_open` - window.open() captured
- `test_popup_policy_block` - Popup blocked by policy

### 7. Clipboard Support

**RESOLVED when:**

```
✓ Copy selection to clipboard
✓ Paste from clipboard
✓ Format negotiation (text, HTML, image)
✓ Context menu copy/paste works
✓ Ctrl+C/Ctrl+V keyboard shortcuts
```

**Acceptance Tests:**
- `test_clipboard_copy_text` - Copy text to clipboard
- `test_clipboard_paste_text` - Paste text from clipboard
- `test_clipboard_html` - Copy/paste HTML format

### 8. DevTools/Web Inspector (MVP Minimal)

**MVP scope (basic debugging):**

```
✓ Console log capture and display
✓ Network request log
✓ DOM tree snapshot/inspection
```

**Full DevTools deferred to post-MVP.**

### 9. Print Functionality

**Deferred to post-MVP** unless explicitly required.

---

## Acceptance Test Matrix

| Test ID | Category | Description | Automation | Priority |
|---------|----------|-------------|------------|----------|
| `T001` | Resize | Basic resize updates content | `hiwave-smoke` | P0 |
| `T002` | Resize | Rapid resize stress (10,000x) | `hiwave-smoke` | P0 |
| `T003` | Resize | Sidebar toggle resize | `hiwave-smoke` | P0 |
| `T004` | MultiView | 3 views render concurrently | `hiwave-smoke` | P0 |
| `T005` | MultiView | Create/destroy 100 views | `hiwave-smoke` | P0 |
| `T006` | Events | Load event ordering | Unit test | P0 |
| `T007` | Events | Error event on failure | Unit test | P0 |
| `T008` | Intercept | Block request at network | Unit test | P0 |
| `T009` | Intercept | Resource type classification | Unit test | P0 |
| `T010` | Download | Basic file download | Integration | P1 |
| `T011` | Download | Progress callbacks | Integration | P1 |
| `T012` | Popup | target="_blank" handling | Unit test | P1 |
| `T013` | Clipboard | Copy/paste text | Integration | P2 |
| `T014` | Stability | 1-hour navigation stress | `hiwave-smoke` | P0 |
| `T015` | Stability | Clean shutdown | Unit test | P0 |

---

## Platform Constraints (MVP)

| Constraint | Decision | Rationale |
|------------|----------|-----------|
| **OS Support** | Windows only | Focus on WinCairo pain points |
| **Architecture** | x64 only | Simplify initial build |
| **GPU** | DirectX 11+ or DirectComposition | Modern Windows graphics stack |
| **Sandbox** | Off for MVP | Reduce complexity; add in hardening phase |

---

## Exit Criteria Summary

The Rust WebKit rewrite MVP is **COMPLETE** when all of the following are true:

1. ✅ All Tier 1 target sites render correctly
2. ✅ All P0 Web APIs implemented and tested
3. ✅ All P0 acceptance tests pass
4. ✅ Performance targets met for resize and paint
5. ✅ 1-hour stress test passes without crash
6. ✅ HiWave Content WebView can use new engine via feature flag
7. ✅ Fallback to WebView2 works on engine failure

---

## Appendix: Test Commands

```bash
# Run all P0 acceptance tests
cargo test --workspace --features rustkit

# Run smoke harness for resize/multiview stress
cargo run -p hiwave-smoke --release -- --duration-ms 60000

# Run CI validation via orchestrator
python tools/ai-orchestrator/aiorch.py ci run --work-order reqs-mvp-matrix

# Run canary validation
python tools/ai-orchestrator/aiorch.py canary run --profile release --duration-ms 10000
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: reqs-mvp-matrix*

