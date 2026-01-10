# WebKit Fork Setup for HiWave

This document describes the WebKit fork strategy, build setup, and test harness for the HiWave Rust browser engine rewrite.

## Overview

The WebKit fork serves as:
1. **Specification-by-example**: LayoutTests + WPT subset define expected behavior
2. **Compatibility target**: Validate against WebKit's observable behavior
3. **Regression harness**: Build equivalent tests for WinCairo limitations

## Fork Strategy

### Repository Structure

```
P:\WebKit/                          # Main WebKit fork
├── Source/
│   ├── WebKit/                     # WebKit2 API layer
│   ├── WebCore/                    # Core rendering engine
│   └── WTF/                        # WebKit Template Framework
├── Tools/
│   └── MiniBrowser/win/            # Reference WinCairo browser
└── LayoutTests/                    # Web platform tests

P:\petes_code\ClaudeCode\hiwave-windows/
├── deps/wincairo/                  # Pre-built WebKit binaries
├── crates/webkit-wincairo/         # Rust bindings
├── crates/webkit-wincairo-sys/     # FFI bindings
└── scripts/
    └── build-webkit.ps1            # Build automation
```

### Sync Strategy

1. **Upstream tracking**: Track WebKit main branch weekly
2. **Patch management**: Maintain local patches in `patches/` directory
3. **Version pinning**: Pin to specific WebKit revision for stability

```powershell
# Sync with upstream (weekly)
cd P:\WebKit
git fetch upstream
git merge upstream/main --no-commit
# Resolve conflicts, run tests, commit
```

## Minimal Reproductions

### 1. Resize Does Not Update Content

**Location**: `WebKit/Source/WebKit/UIProcess/win/WebView.cpp`

**Reproduction Steps**:
```cpp
// MiniBrowser modification to reproduce:
// 1. Create WebView with initial size 800x600
// 2. Load any HTML page
// 3. Resize window to 1024x768
// 4. Observe: HWND resizes, content stays 800x600
```

**Minimal Test Case** (`tests/resize-repro.html`):
```html
<!DOCTYPE html>
<html>
<head>
  <style>
    body { margin: 0; }
    #marker {
      position: fixed;
      right: 10px;
      bottom: 10px;
      width: 50px;
      height: 50px;
      background: red;
    }
  </style>
</head>
<body>
  <div id="marker"></div>
  <script>
    // Marker should always be 10px from bottom-right
    // After resize, marker position should update
    setInterval(() => {
      const rect = document.getElementById('marker').getBoundingClientRect();
      console.log(`Marker: right=${window.innerWidth - rect.right}, bottom=${window.innerHeight - rect.bottom}`);
    }, 500);
  </script>
</body>
</html>
```

**Root Cause Analysis**:
- `WebView::setViewSize()` updates HWND but not compositor surface
- Accelerated compositing surface is created at fixed size
- `WM_SIZE` handler doesn't trigger surface recreation

**Key Files to Investigate**:
```
Source/WebKit/UIProcess/win/WebView.cpp
  - WebView::setViewSize()
  - WebView::windowReceivedMessage() [WM_SIZE handler]

Source/WebCore/platform/graphics/win/GraphicsLayerDirect2D.cpp
  - Surface creation and sizing

Source/WebKit/UIProcess/CoordinatedGraphics/DrawingAreaProxyCoordinatedGraphics.cpp
  - Coordinated graphics sizing
```

### 2. Multiple WebViews Fail

**Location**: Global state in compositor/GPU context

**Reproduction Steps**:
```cpp
// 1. Create WebView A, load page (renders correctly)
// 2. Create WebView B, load different page
// 3. Observe: WebView A goes blank, only B renders
```

**Minimal Test Case**:
```cpp
// MiniBrowser modification:
HWND parent = CreateWindow(...);
WKViewRef viewA = WKViewCreate(parent, ...);
WKPageLoadURL(WKViewGetPage(viewA), urlA);
// Wait for load...

WKViewRef viewB = WKViewCreate(parent, ...);
WKPageLoadURL(WKViewGetPage(viewB), urlB);
// Now viewA is blank!
```

**Root Cause Hypothesis**:
- Shared GPU device/context between views
- Last-created view takes exclusive ownership
- No per-view resource isolation

**Key Files to Investigate**:
```
Source/WebCore/platform/graphics/win/PlatformContextDirect2D.cpp
  - D2D device/context management

Source/WebKit/UIProcess/win/WebPageProxyWin.cpp
  - Page creation and GPU resource allocation
```

### 3. Page Load Callbacks Not Firing

**Location**: `WKPageLoaderClient` registration

**Reproduction Steps**:
```cpp
WKPageLoaderClientV0 loaderClient = {
    .didStartProvisionalLoadForFrame = myStartCallback,
    .didCommitLoadForFrame = myCommitCallback,
    .didFinishLoadForFrame = myFinishCallback,
    .didFailLoadWithErrorForFrame = myFailCallback,
};
WKPageSetPageLoaderClient(page, &loaderClient.base);
WKPageLoadURL(page, url);
// Callbacks never fire!
```

**Expected vs Actual**:
| Event | Expected | Actual (WinCairo) |
|-------|----------|-------------------|
| didStartProvisionalLoad | ✓ | ✗ Not called |
| didCommitLoad | ✓ | ✗ Not called |
| didFinishLoad | ✓ | ✗ Not called |
| didFailLoad | ✓ | ✗ Not called |

**Key Files to Investigate**:
```
Source/WebKit/UIProcess/API/C/WKPage.cpp
  - WKPageSetPageLoaderClient()

Source/WebKit/UIProcess/WebPageProxy.cpp
  - didStartProvisionalLoadForFrame()
  - didCommitLoadForFrame()
```

### 4. Navigation Interception Missing

**Location**: `WKPageNavigationClient`

**Current State**:
- `decidePolicyForNavigationAction` exists but unreliable
- Navigation decisions don't reach embedder consistently

**Workaround** (current HiWave approach):
```javascript
// JS-based interception (slower, incomplete)
document.addEventListener('click', (e) => {
  if (e.target.tagName === 'A') {
    e.preventDefault();
    window.ipc.postMessage(JSON.stringify({
      cmd: 'navigate',
      url: e.target.href
    }));
  }
});
```

### 5. Download Handling Missing

**Location**: Download delegate callbacks

**Expected API**:
```cpp
WKDownloadClientV0 downloadClient = {
    .didStart = myDownloadStart,
    .didReceiveData = myDownloadProgress,
    .didFinish = myDownloadComplete,
    .didFail = myDownloadFail,
};
```

**Actual State**: Callbacks not implemented for WinCairo

## LayoutTests Subset

### Priority Tests for MVP

```bash
# Resize-related tests
LayoutTests/fast/events/resize*.html
LayoutTests/css3/viewport/*.html

# Multi-frame tests
LayoutTests/fast/frames/*.html
LayoutTests/fast/loader/*.html

# Navigation tests
LayoutTests/fast/history/*.html
LayoutTests/http/tests/navigation/*.html

# Download tests
LayoutTests/http/tests/download/*.html
```

### Running LayoutTests

```powershell
# From WebKit root
cd P:\WebKit

# Run specific test
python Tools/Scripts/run-webkit-tests --wincairo fast/events/resize-basic.html

# Run subset
python Tools/Scripts/run-webkit-tests --wincairo --test-list=tests/hiwave-mvp.txt
```

### HiWave MVP Test List (`tests/hiwave-mvp.txt`)

```
fast/events/resize-event.html
fast/events/resize-subframe.html
fast/frames/frame-navigation.html
fast/loader/document-load-callbacks.html
http/tests/navigation/redirect-chain.html
http/tests/download/basic-download.html
```

## Build Configuration

### Prerequisites

1. **Visual Studio 2022** with:
   - Desktop development with C++
   - Windows SDK 10.0.22621.0+
   - CMake tools

2. **Dependencies**:
   - Python 3.9+
   - Ruby 3.0+
   - Perl 5.30+
   - Git

### Build Script

See `scripts/build-webkit.ps1` for automated build.

Quick build:
```powershell
cd P:\petes_code\ClaudeCode\hiwave-windows
.\scripts\build-webkit.ps1 -Config Release -Target MiniBrowser
```

### Build Output

```
P:\WebKit\WebKitBuild\Release\bin64\
├── MiniBrowser.exe          # Test browser
├── WebKit2.dll              # WebKit2 API
├── WebCore.dll              # Core engine
└── *.pdb                    # Debug symbols
```

## Continuous Integration

### Local CI Workflow

```yaml
# .github/workflows/webkit-ci.yml (local simulation)
name: WebKit WinCairo CI

on: [push, pull_request]

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
        with:
          repository: petes_code/webkit-fork
          
      - name: Build WinCairo
        run: |
          .\scripts\build-webkit.ps1 -Config Release
          
      - name: Run Layout Tests
        run: |
          python Tools/Scripts/run-webkit-tests --wincairo --test-list=tests/hiwave-mvp.txt
```

### Local Simulation

```powershell
# Simulate CI locally
python tools/ai-orchestrator/aiorch.py ci run --work-order webkit-fork-ci
```

## Next Steps

1. **Fork Creation**: Set up GitHub fork with CI
2. **Patch Development**: Create patches for identified issues
3. **Test Automation**: Integrate LayoutTests into canary runner
4. **Upstream Contribution**: Submit fixes back to WebKit project

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: webkit-fork-ci*

