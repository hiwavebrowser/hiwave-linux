# WinCairo WebKit Limitations for HiWave

This document details the current limitations of WebKit's WinCairo port that prevent full browser functionality in HiWave. These issues need to be addressed either through upstream WebKit contributions or workarounds.

## Critical Blockers

### 1. View Resize Does Not Update Rendered Content

**Status:** BLOCKING
**Severity:** Critical
**Location:** `WebKitView::set_bounds()` in `crates/webkit-wincairo/src/view.rs`

**Problem:**
When the WebKit view's HWND is resized via `SetWindowPos()`, the Windows window changes size correctly, but WebKit's internal rendering surface does not update. The accelerated compositing layer ignores HWND size changes.

**Symptoms:**
- Sidebar panels open/close correctly at the OS level
- WebKit content remains at original size, causing overlap or hidden content
- `SetWindowPos()` returns success (1)
- `WM_SIZE` message is sent but has no visible effect

**Attempted Fixes (all failed):**
1. `InvalidateRect()` + `UpdateWindow()` - No effect
2. `WKViewWindowAncestryDidChange()` - No effect
3. `SendMessageW(hwnd, WM_SIZE, ...)` - No effect
4. `WKPageForceRepaint()` - No effect
5. Disabling accelerated compositing - Breaks rendering entirely

**Root Cause:**
WinCairo's accelerated compositing implementation creates a DirectX/OpenGL surface that is sized at creation time. The surface does not respond to HWND resize events. This appears to be a fundamental architectural limitation in the WinCairo port.

**Impact:**
- Sidebars cannot be used (content doesn't resize)
- Window resize may not work properly
- Any dynamic layout changes fail

**Potential Fix Locations in WebKit Source:**
- `Source/WebKit/UIProcess/win/WebView.cpp` - Window message handling
- `Source/WebCore/platform/graphics/win/` - Accelerated compositing layer
- `Source/WebKit/UIProcess/CoordinatedGraphics/` - Coordinated graphics system

---

### 2. Multiple Concurrent WebViews Not Supported

**Status:** BLOCKING (worked around with hybrid architecture)
**Severity:** Critical

**Problem:**
Creating more than one WKView instance causes the first view to stop rendering. Only the most recently created view renders correctly.

**Symptoms:**
- First WebView goes blank when second is created
- Only one WebView can render at a time
- No errors reported - silent failure

**Current Workaround:**
Hybrid architecture using WRY/WebView2 for Chrome panels and WinCairo WebKit for content only. This limits us to a single WebKit view for the main content area.

**Root Cause:**
Unknown. May be related to shared GPU resources or context management in the accelerated compositing system.

---

## Major Issues

### 3. Page Load Events Not Implemented

**Status:** Not Working
**Location:** `WKPageLoaderClient` callbacks

**Problem:**
The page loader client callbacks (`didStartProvisionalLoadForFrame`, `didCommitLoadForFrame`, `didFinishLoadForFrame`, `didFailLoadWithErrorForFrame`) are either not called or not properly wired up in WinCairo.

**Impact:**
- Loading spinner UI doesn't work
- Progress indicators don't update
- `is_loading()` may return incorrect values

**What's Needed:**
- Verify callback registration in `WKPageSetPageLoaderClient()`
- Check if WinCairo implements these callbacks
- May need to implement callbacks in the WebKit source

---

### 4. Navigation Interception Limited

**Status:** Partial (JS workaround only)

**Problem:**
`WKPageNavigationClient` for intercepting navigation decisions is not reliably called on WinCairo.

**Current Workaround:**
JavaScript-based navigation interception using `beforeunload` and link click handlers.

**Impact:**
- Shield ad-blocking cannot intercept requests at network level
- Must rely on JS injection which is slower and less reliable
- Some navigation events may slip through

---

### 5. Download Handlers Not Implemented

**Status:** Not Working

**Problem:**
Download-related callbacks (`decidePolicyForNavigationAction` with download response, `didReceiveResponse` for downloads) are not implemented.

**Impact:**
- Files download directly without user prompt
- No download progress tracking
- No download management UI

---

## Minor Issues

### 6. New Window/Popup Handling

**Status:** Not Working

**Problem:**
`createNewPage` callback in `WKPageUIClient` doesn't function on WinCairo.

**Impact:**
- Popups open in the same window or fail silently
- No popup blocking capability
- `target="_blank"` links may not work correctly

---

### 7. Clipboard Support

**Status:** Limited

**Problem:**
Copy/paste operations via `WKPageExecuteCommand("copy"/"paste")` are unreliable.

**Impact:**
- User clipboard operations may fail
- Context menu copy/paste may not work

---

### 8. DevTools/Web Inspector

**Status:** Not Working

**Problem:**
`WKPageGetInspector()` returns a valid pointer but `WKInspectorShow()` has no effect.

**Impact:**
- No developer tools for debugging
- Cannot inspect page content

---

### 9. Print Functionality

**Status:** Stub Only

**Problem:**
`WKPageBeginPrinting()` is implemented but actual print output doesn't work.

**Impact:**
- Print feature non-functional

---

## API Coverage Summary

| Feature | Status | Notes |
|---------|--------|-------|
| Basic page loading | Working | `WKPageLoadURL`, `WKPageLoadHTMLString` |
| JavaScript execution | Working | `WKPageEvaluateJavaScriptInMainFrame` |
| Navigation (back/forward) | Working | `WKPageGoBack`, `WKPageGoForward` |
| User scripts | Working | `WKUserContentControllerAddUserScript` |
| Script message handlers | Working | `WKUserContentControllerAddScriptMessageHandler` |
| Page zoom | Working | `WKPageSetPageZoomFactor` |
| View resize | **NOT WORKING** | HWND resizes, content doesn't |
| Multiple views | **NOT WORKING** | Only one view renders |
| Page load events | Not Working | Callbacks not firing |
| Navigation interception | Partial | JS workaround only |
| Downloads | Not Working | No handler callbacks |
| Popups/new windows | Not Working | No `createNewPage` support |
| DevTools | Not Working | Inspector doesn't show |
| Print | Not Working | Stub implementation |

---

## WebKit Source Files to Investigate

For fixing the resize issue, these are the key files in the WebKit source:

```
WebKit/Source/WebKit/UIProcess/win/
├── WebView.cpp              # Main view implementation
├── WebView.h
├── PageClientImpl.cpp       # Page client for UI events
└── WebPageProxyWin.cpp      # Proxy to web process

WebKit/Source/WebCore/platform/graphics/win/
├── GraphicsLayerDirect2D.cpp    # Accelerated compositing
├── MediaPlayerPrivateWin.cpp
└── TextureMapperWin.cpp         # Texture mapping

WebKit/Source/WebKit/UIProcess/CoordinatedGraphics/
├── DrawingAreaProxyCoordinatedGraphics.cpp
└── CoordinatedDrawingArea.cpp
```

---

## Recommended Fix Priority

1. **View Resize** - Without this, the browser is barely usable
2. **Page Load Events** - Needed for proper loading UI
3. **Navigation Interception** - Needed for ad blocking
4. **Multiple Views** - Would eliminate need for hybrid architecture

---

## Testing Methodology

When testing fixes:

1. Build with `cargo build --release --features wincairo`
2. Run `target/release/hiwave.exe`
3. Test resize by opening/closing sidebars
4. Check console output for WebKit logs
5. Use MiniBrowser.exe (in deps/wincairo/) as reference implementation

---

## Resources

- WebKit WinCairo Bug Tracker: https://bugs.webkit.org/ (search "WinCairo")
- WebKit Source Browser: https://trac.webkit.org/browser/webkit/trunk
- MiniBrowser Source: `deps/wincairo/WebKit/Tools/MiniBrowser/win/`
