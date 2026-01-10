# HiWave macOS Port - Planning Document

## Executive Summary

Porting HiWave from Windows to macOS requires adapting platform-specific components while retaining the ~90% of code that is already cross-platform Rust. The main work involves replacing Win32/DirectWrite APIs with Cocoa/Core Text equivalents.

**Estimated Effort:** 3-6 weeks for a full-featured port  
**Complexity:** Medium - most engine code is platform-agnostic

---

## Current Architecture Analysis

### Platform-Agnostic Components (No Changes Needed)

These crates work on any platform:

| Crate | Lines | Status |
|-------|-------|--------|
| `rustkit-html` | ~2,500 | ✅ Pure Rust |
| `rustkit-dom` | ~1,800 | ✅ Pure Rust |
| `rustkit-css` | ~3,000 | ✅ Pure Rust |
| `rustkit-cssparser` | ~1,200 | ✅ Pure Rust |
| `rustkit-layout` | ~2,800 | ✅ Pure Rust |
| `rustkit-js` | ~800 | ✅ Pure Rust (Boa) |
| `rustkit-bindings` | ~600 | ✅ Pure Rust |
| `rustkit-http` | ~700 | ✅ Uses native-tls (cross-platform) |
| `rustkit-net` | ~1,500 | ✅ Pure Rust |
| `rustkit-codecs` | ~400 | ✅ Pure Rust |
| `rustkit-engine` | ~800 | ✅ Pure Rust |
| `rustkit-animation` | ~500 | ✅ Pure Rust |
| `rustkit-svg` | ~600 | ✅ Pure Rust |
| `rustkit-canvas` | ~700 | ✅ Pure Rust |
| `rustkit-webgl` | ~500 | ✅ Uses wgpu (cross-platform) |
| `rustkit-idb` | ~400 | ✅ Pure Rust |
| `rustkit-sw` | ~500 | ✅ Pure Rust |
| `rustkit-worker` | ~400 | ✅ Pure Rust |
| `rustkit-media` | ~600 | ⚠️ Uses rodio (mostly cross-platform) |
| `rustkit-a11y` | ~400 | ❌ Windows UI Automation |
| `rustkit-core` | ~1,200 | ✅ Pure Rust |
| `rustkit-common` | ~300 | ✅ Pure Rust |

**Total Platform-Agnostic:** ~21,400 lines (~85% of engine)

### Platform-Specific Components (Need Porting)

| Crate | Lines | Windows API | macOS Equivalent |
|-------|-------|-------------|------------------|
| `rustkit-viewhost` | ~1,300 | Win32 HWND, WM_* messages | NSView, NSWindow |
| `rustkit-text` | ~600 | DirectWrite | Core Text |
| `rustkit-compositor` | ~400 | Win32 surface | CALayer/Metal surface |
| `rustkit-renderer` | ~800 | wgpu + DirectWrite | wgpu + Core Text |
| `rustkit-a11y` | ~400 | UI Automation | NSAccessibility |
| `hiwave-app` | ~7,000 | Win32 window, tao | tao (already abstracted) |

**Total Platform-Specific:** ~10,500 lines (~15% of engine)

---

## Porting Phases

### Phase 1: Build System & CI (1-2 days)

**Goal:** Get the project building on macOS

1. **Cargo.toml Updates**
   - Add `#[cfg(target_os = "macos")]` conditionals
   - Add macOS-specific dependencies:
     ```toml
     [target.'cfg(target_os = "macos")'.dependencies]
     cocoa = "0.25"
     core-foundation = "0.9"
     core-text = "20.1"
     objc = "0.2"
     ```

2. **CI Pipeline**
   - Add macOS runner to GitHub Actions
   - Build matrix: Windows x64, macOS Intel, macOS ARM64

3. **Stub Implementations**
   - Create `rustkit-viewhost/src/macos.rs` (stub)
   - Create `rustkit-text/src/macos.rs` (stub)

**Deliverable:** `cargo build` succeeds on macOS with stub implementations

---

### Phase 2: Text Rendering - Core Text (3-4 days)

**Goal:** Replace DirectWrite with Core Text for text shaping

**Files to Create:**
- `crates/rustkit-text/src/macos.rs`

**API Mapping:**

| DirectWrite | Core Text |
|-------------|-----------|
| `IDWriteFactory` | `CTFontDescriptor` |
| `IDWriteTextFormat` | `CTFont` |
| `IDWriteTextLayout` | `CTLine`, `CTFrame` |
| `IDWriteFontCollection` | `CTFontCollection` |
| Font fallback | `CTFontCreateForString` |
| Glyph runs | `CTRunGetGlyphs` |
| Metrics | `CTFontGetAscent`, `CTFontGetDescent` |

**Implementation:**
```rust
// crates/rustkit-text/src/macos.rs
use core_text::font::CTFont;
use core_text::line::CTLine;
use core_foundation::string::CFString;

pub struct TextShaper {
    font: CTFont,
}

impl TextShaper {
    pub fn shape(&self, text: &str) -> ShapedText {
        let cf_string = CFString::new(text);
        let line = CTLine::new_with_attributed_string(...);
        // Extract glyphs, positions, advances
    }
}
```

**Tests:**
- Glyph positioning matches expected values
- Font fallback works for emoji and CJK
- Correct metrics for common fonts

---

### Phase 3: ViewHost - NSView (5-7 days)

**Goal:** Replace Win32 HWND with NSView for view hosting

**Files to Create:**
- `crates/rustkit-viewhost/src/macos.rs`

**API Mapping:**

| Win32 | Cocoa |
|-------|-------|
| `HWND` | `NSView` |
| `CreateWindowExW` | `NSView::initWithFrame:` |
| `WM_SIZE` | `NSViewFrameDidChangeNotification` |
| `WM_PAINT` | `drawRect:` |
| `WM_MOUSEMOVE` | `mouseMoved:` |
| `WM_KEYDOWN` | `keyDown:` |
| `WM_SETFOCUS` | `becomeFirstResponder` |
| `GetDpiForWindow` | `backingScaleFactor` |
| `TrackMouseEvent` | `NSTrackingArea` |

**Key Classes:**
```rust
// crates/rustkit-viewhost/src/macos.rs
use cocoa::base::{id, nil};
use cocoa::appkit::{NSView, NSWindow};
use objc::runtime::{Class, Object};

pub struct MacOSViewHost {
    view: id,        // NSView
    layer: id,       // CAMetalLayer
    scale_factor: f64,
}

impl ViewHost for MacOSViewHost {
    fn create(parent: RawWindowHandle) -> Result<Self, ViewHostError> {
        // Create NSView subclass with custom event handlers
        // Attach CAMetalLayer for wgpu surface
    }
    
    fn resize(&mut self, width: u32, height: u32) {
        // Update frame, notify compositor
    }
    
    fn handle_events(&mut self) -> Vec<InputEvent> {
        // Convert NSEvent to InputEvent
    }
}
```

**Challenges:**
- Objective-C runtime interop via `objc` crate
- Custom NSView subclass for event handling
- CAMetalLayer setup for wgpu

---

### Phase 4: Compositor & Renderer (2-3 days)

**Goal:** GPU surface creation for macOS

**Changes:**
- `rustkit-compositor`: Use `wgpu` with Metal backend
- `rustkit-renderer`: Replace DirectWrite glyph rendering with Core Text

**wgpu Surface:**
```rust
// wgpu automatically selects Metal on macOS
let surface = instance.create_surface(&window)?;
```

**Glyph Rendering:**
- Core Text provides glyph outlines or rasterized bitmaps
- Integrate with `rustkit-renderer` glyph cache

---

### Phase 5: Accessibility - NSAccessibility (3-4 days)

**Goal:** Replace UI Automation with NSAccessibility

**API Mapping:**

| UI Automation | NSAccessibility |
|---------------|-----------------|
| `IRawElementProviderSimple` | `NSAccessibility` protocol |
| `UIA_ButtonControlTypeId` | `NSAccessibilityRole.button` |
| `IAccessible::get_accName` | `accessibilityLabel` |
| `IAccessible::get_accRole` | `accessibilityRole` |
| Focus events | `NSAccessibilityFocusedUIElementChangedNotification` |

---

### Phase 6: HiWave App Integration (3-4 days)

**Goal:** Full app running on macOS

**Already Done:**
- `platform/macos.rs` exists with menu setup
- `tao` handles window creation cross-platform
- `wry` handles chrome WebView cross-platform

**Remaining Work:**
1. Replace RustKit viewhost in content area
2. Icon generation for macOS (ICNS format)
3. App bundle structure (`HiWave.app/Contents/...`)
4. Code signing and notarization setup
5. DMG installer creation

**App Bundle Structure:**
```
HiWave.app/
├── Contents/
│   ├── Info.plist
│   ├── MacOS/
│   │   └── hiwave
│   ├── Resources/
│   │   ├── hiwave.icns
│   │   └── assets/
│   └── Frameworks/
```

---

### Phase 7: Testing & Polish (3-5 days)

1. **Unit Tests**
   - Run full test suite on macOS
   - Platform-specific test cases

2. **Integration Tests**
   - Navigation, rendering, input
   - Clipboard operations
   - File dialogs

3. **Visual Testing**
   - Retina display support
   - Font rendering quality
   - Color accuracy

4. **Performance**
   - Benchmark vs Windows
   - Memory profiling
   - Energy impact (Battery)

---

## Dependencies to Add

```toml
# Cargo.toml - macOS-specific
[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.25"
core-foundation = "0.9"
core-text = "20.1"
core-graphics = "0.23"
objc = "0.2"
objc2 = "0.5"  # Modern objc bindings
dispatch = "0.2"
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Core Text API differences | Medium | Medium | Early prototyping |
| NSView event complexity | Medium | High | Study Safari/Firefox approach |
| Metal surface issues | Low | Medium | wgpu abstracts most of this |
| Retina scaling bugs | Medium | Low | Test on HiDPI displays |
| Code signing issues | Medium | Low | Apple Developer account |

---

## Timeline Estimate

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| 1. Build System | 1-2 days | None |
| 2. Text (Core Text) | 3-4 days | Phase 1 |
| 3. ViewHost (NSView) | 5-7 days | Phase 1 |
| 4. Compositor | 2-3 days | Phases 2, 3 |
| 5. Accessibility | 3-4 days | Phases 3, 4 |
| 6. App Integration | 3-4 days | All above |
| 7. Testing | 3-5 days | Phase 6 |

**Total: 3-6 weeks** (depending on complexity discoveries)

---

## Recommended Approach

### Option A: Parallel Development (Recommended)
- Create `src/macos.rs` files alongside `src/win.rs`
- Use `#[cfg(target_os)]` for conditional compilation
- Same crate, multiple platform implementations

### Option B: Platform Crates
- `rustkit-viewhost-win32`
- `rustkit-viewhost-macos`
- More separation but more duplication

### Decision: Option A
- Keeps code together
- Easier to maintain feature parity
- Standard Rust pattern

---

## Success Criteria

1. ✅ `cargo build --target x86_64-apple-darwin` succeeds
2. ✅ `cargo test` passes on macOS
3. ✅ HiWave.app launches and displays content
4. ✅ Text renders correctly (Latin, CJK, Emoji)
5. ✅ Mouse/keyboard input works
6. ✅ Scrolling is smooth
7. ✅ VoiceOver accessibility works
8. ✅ DMG installer works

---

## Resources

- [Core Text Programming Guide](https://developer.apple.com/library/archive/documentation/StringsTextFonts/Conceptual/CoreText_Programming/)
- [NSView Documentation](https://developer.apple.com/documentation/appkit/nsview)
- [wgpu Metal Backend](https://wgpu.rs/)
- [rust-objc Guide](https://docs.rs/objc/latest/objc/)
- [Servo's Core Text Code](https://github.com/nicegram/nicegram-ios/tree/main/nicegram-nicegram-rust) (reference implementation)

---

## Next Steps

1. **Set up macOS development environment**
   - Install Xcode command line tools
   - Configure cross-compilation (if developing on Windows)

2. **Create tracking issue/project board**
   - Break phases into individual tasks
   - Assign priorities

3. **Start with Phase 1**
   - Get basic build working
   - Create stub implementations

4. **Prototype Core Text** (can be done in parallel)
   - Simple text shaping test
   - Validate approach before full implementation

