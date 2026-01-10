# RustKit Phases 14-20: Core Browser Features Plan

**Created:** January 2, 2026
**Status:** Planning
**Prerequisites:** Phases 0-13 must be complete

---

## Overview

Phases 14-20 transform RustKit from a static HTML/CSS renderer into an interactive browser capable of handling user input, forms, media, modern layouts, scrolling, navigation, and security. These are the **critical phases** that make a browser actually usable.

**Total Estimated Effort:** 6-9 months with a small team

---

## Phase Dependencies Graph

```
Phase 12 (Box Model) ──┬──→ Phase 14 (Events)
                       └──→ Phase 17 (Flexbox)

Phase 13 (Text) ───────────→ Phase 14 (Events)

Phase 14 (Events) ─────┬──→ Phase 15 (Forms)
                       ├──→ Phase 16 (Images)
                       └──→ Phase 18 (Scrolling)

Phase 15 (Forms) ──────────→ Phase 19 (Navigation)

Phase 16 (Images) ─────────→ Phase 19 (Navigation)

Phase 18 (Scrolling) ──────→ Phase 19 (Navigation)

Phase 19 (Navigation) ─────→ Phase 20 (Security)
```

---

## Phase 14: Event Handling

### Overview
Implement the complete DOM event system including mouse, keyboard, focus, and touch events. This is the foundation for all interactivity.

### Priority: Critical
### Estimated Duration: 4-5 weeks
### Dependencies: Phase 12 (Box Model), Phase 13 (Text)

### Sub-Tasks

#### 14.1 Hit Testing
- [x] Point-in-box detection
- [x] Z-index aware hit testing (topmost element first)
- [x] Hit test result with local coordinates
- [x] Ancestor chain tracking
- [ ] Pointer-events CSS property (`none`, `auto`, `visiblePainted`)
- [ ] Hit testing through transparent elements
- [ ] SVG hit testing (future, Phase 23)

#### 14.2 Mouse Events
- [ ] `mousedown` - button pressed
- [ ] `mouseup` - button released
- [ ] `click` - press and release
- [ ] `dblclick` - double click
- [ ] `mousemove` - cursor movement
- [ ] `mouseenter` / `mouseleave` - enter/leave element (no bubble)
- [ ] `mouseover` / `mouseout` - enter/leave with bubbling
- [ ] `contextmenu` - right-click

##### MouseEvent Properties
```rust
pub struct MouseEvent {
    // Coordinates
    pub client_x: f64,      // Relative to viewport
    pub client_y: f64,
    pub screen_x: f64,      // Relative to screen
    pub screen_y: f64,
    pub offset_x: f64,      // Relative to target element
    pub offset_y: f64,
    pub page_x: f64,        // Relative to document
    pub page_y: f64,
    pub movement_x: f64,    // Delta from last event
    pub movement_y: f64,

    // Button state
    pub button: i16,        // Which button (0=left, 1=middle, 2=right)
    pub buttons: u16,       // Bitmask of pressed buttons

    // Modifiers
    pub ctrl_key: bool,
    pub alt_key: bool,
    pub shift_key: bool,
    pub meta_key: bool,

    // Target
    pub target: NodeId,
    pub related_target: Option<NodeId>,  // For enter/leave
}
```

#### 14.3 Keyboard Events
- [ ] `keydown` - key pressed
- [ ] `keyup` - key released
- [ ] `keypress` (deprecated but needed for compat)

##### KeyboardEvent Properties
```rust
pub struct KeyboardEvent {
    pub key: String,        // "a", "Enter", "ArrowUp", etc.
    pub code: String,       // "KeyA", "Enter", "ArrowUp" (physical key)
    pub location: u32,      // 0=standard, 1=left, 2=right, 3=numpad
    pub repeat: bool,       // Auto-repeat
    pub is_composing: bool, // IME composition

    // Modifiers
    pub ctrl_key: bool,
    pub alt_key: bool,
    pub shift_key: bool,
    pub meta_key: bool,
}
```

#### 14.4 Focus Events
- [ ] `focus` - element gained focus (no bubble)
- [ ] `blur` - element lost focus (no bubble)
- [ ] `focusin` - focus with bubbling
- [ ] `focusout` - blur with bubbling

##### Focus Management
- [ ] `tabindex` attribute support
- [ ] Natural tab order (DOM order)
- [ ] Tab/Shift+Tab navigation
- [ ] `document.activeElement`
- [ ] `element.focus()` and `element.blur()` methods
- [ ] Focus trapping for modals
- [ ] `:focus` and `:focus-visible` pseudo-classes
- [ ] `:focus-within` pseudo-class

#### 14.5 Input Events
- [ ] `input` - value changed
- [ ] `change` - value committed (blur or enter)
- [ ] `beforeinput` - before value changes

##### InputEvent Properties
```rust
pub struct InputEvent {
    pub data: Option<String>,   // Inserted text
    pub input_type: String,     // "insertText", "deleteContentBackward", etc.
    pub is_composing: bool,
}
```

#### 14.6 Composition Events (IME)
- [ ] `compositionstart`
- [ ] `compositionupdate`
- [ ] `compositionend`

#### 14.7 Pointer Events (Modern API)
- [ ] `pointerdown`, `pointerup`, `pointermove`
- [ ] `pointerenter`, `pointerleave`
- [ ] `pointerover`, `pointerout`
- [ ] `pointercancel`
- [ ] `gotpointercapture`, `lostpointercapture`

##### PointerEvent Properties
```rust
pub struct PointerEvent {
    // Inherits MouseEvent properties plus:
    pub pointer_id: i32,
    pub width: f64,
    pub height: f64,
    pub pressure: f32,
    pub tilt_x: i32,
    pub tilt_y: i32,
    pub pointer_type: String,  // "mouse", "pen", "touch"
    pub is_primary: bool,
}
```

#### 14.8 Touch Events
- [ ] `touchstart`
- [ ] `touchmove`
- [ ] `touchend`
- [ ] `touchcancel`

##### TouchEvent and Touch
```rust
pub struct TouchEvent {
    pub touches: Vec<Touch>,         // All active touches
    pub target_touches: Vec<Touch>,  // Touches on target
    pub changed_touches: Vec<Touch>, // Touches that changed
}

pub struct Touch {
    pub identifier: i64,
    pub target: NodeId,
    pub client_x: f64,
    pub client_y: f64,
    pub screen_x: f64,
    pub screen_y: f64,
    pub page_x: f64,
    pub page_y: f64,
    pub radius_x: f64,
    pub radius_y: f64,
    pub rotation_angle: f64,
    pub force: f64,
}
```

#### 14.9 Event Dispatch System
- [x] Event object creation with properties
- [x] `preventDefault()` support
- [x] `stopPropagation()` support
- [x] `stopImmediatePropagation()` support
- [ ] Capture phase (root to target)
- [ ] Target phase
- [ ] Bubble phase (target to root)
- [ ] Event delegation support
- [ ] Passive event listeners
- [ ] `once` option for listeners

##### Event Dispatch Algorithm
```rust
pub struct EventDispatcher {
    // 1. Build propagation path (target → root)
    // 2. Capture phase: root → target, capturing listeners
    // 3. Target phase: target listeners
    // 4. Bubble phase: target → root, bubbling listeners
    // 5. Handle defaultPrevented for default actions
}

impl EventDispatcher {
    pub fn dispatch(
        &self,
        event: &mut Event,
        target: &Node,
        path: &[Rc<Node>],
    ) -> bool {
        // Capture phase
        event.event_phase = EventPhase::Capturing;
        for ancestor in path.iter().rev() {
            self.invoke_listeners(ancestor, event, true);
            if event.propagation_stopped { break; }
        }

        // Target phase
        event.event_phase = EventPhase::AtTarget;
        self.invoke_listeners(target, event, false);

        // Bubble phase
        if event.bubbles && !event.propagation_stopped {
            event.event_phase = EventPhase::Bubbling;
            for ancestor in path.iter() {
                self.invoke_listeners(ancestor, event, false);
                if event.propagation_stopped { break; }
            }
        }

        !event.default_prevented
    }
}
```

#### 14.10 Cursor Management
- [ ] CSS `cursor` property support
- [ ] Cursor types: `default`, `pointer`, `text`, `move`, `grab`, etc.
- [ ] Custom cursors (`url()`)
- [ ] Cursor change on hover

#### 14.11 Platform Integration (Windows)
- [ ] Win32 message handling (`WM_MOUSEMOVE`, `WM_LBUTTONDOWN`, etc.)
- [ ] Win32 keyboard input (`WM_KEYDOWN`, `WM_CHAR`, etc.)
- [ ] IME integration (`WM_IME_*` messages)
- [ ] Touch input (`WM_TOUCH`, `WM_POINTER*`)
- [ ] High-DPI coordinate translation

### Third-Party Libraries
- `windows` crate for Win32 input APIs
- Consider: `winit` for cross-platform input (but we use custom viewhost)

### Custom Implementation Required
```rust
// New module: rustkit-events or extend rustkit-core
pub struct EventLoop {
    pending_events: VecDeque<PlatformEvent>,
    mouse_state: MouseState,
    keyboard_state: KeyboardState,
    focus_manager: FocusManager,
    touch_state: TouchState,
}

pub struct FocusManager {
    focused_element: Option<NodeId>,
    tab_order: Vec<NodeId>,  // Computed from tabindex
}

pub struct MouseState {
    position: (f64, f64),
    buttons: u16,
    hover_target: Option<NodeId>,
    capture_target: Option<NodeId>,
}
```

### Acceptance Criteria
- [ ] Click events fire on buttons
- [ ] Hover states work (`:hover` pseudo-class)
- [ ] Keyboard navigation with Tab works
- [ ] Text selection with mouse works
- [ ] Double-click selects word
- [ ] Context menu (right-click) works
- [ ] Modifier keys (Ctrl, Shift, Alt) detected
- [ ] Touch events work on touch devices
- [ ] Event bubbling works correctly
- [ ] `preventDefault()` stops default actions
- [ ] Pass 50% of WPT `uievents/` tests

### Risk Assessment
- **Platform complexity**: Win32 input handling has many edge cases
- **IME**: Input Method Editors for CJK languages are complex
- **Performance**: High-frequency mouse events must be efficient
- **Focus**: Focus management edge cases (iframes, shadow DOM)

---

## Phase 15: Forms & Input

### Overview
Implement HTML form elements and input handling. Forms are essential for any interactive website.

### Priority: Critical
### Estimated Duration: 4-5 weeks
### Dependencies: Phase 14 (Events)

### Sub-Tasks

#### 15.1 Text Inputs
- [ ] `<input type="text">` - single line text
- [ ] `<input type="password">` - masked input
- [ ] `<input type="email">` - email validation
- [ ] `<input type="url">` - URL validation
- [ ] `<input type="tel">` - telephone input
- [ ] `<input type="search">` - search input with clear button
- [ ] `<input type="number">` - numeric input with spinners
- [ ] `<textarea>` - multi-line text

##### Text Input Features
- [ ] Cursor positioning and movement
- [ ] Text selection (mouse and keyboard)
- [ ] Copy/Cut/Paste (clipboard integration)
- [ ] Undo/Redo
- [ ] Placeholder text
- [ ] `maxlength` attribute
- [ ] `minlength` attribute
- [ ] `readonly` attribute
- [ ] `disabled` attribute
- [ ] `autofocus` attribute
- [ ] `autocomplete` attribute
- [ ] Selection API (`selectionStart`, `selectionEnd`, `setSelectionRange`)

#### 15.2 Button Inputs
- [ ] `<button>` element
- [ ] `<input type="submit">` - form submission
- [ ] `<input type="reset">` - form reset
- [ ] `<input type="button">` - generic button
- [ ] `<input type="image">` - image button
- [ ] Disabled state styling
- [ ] Click and keyboard activation (Enter/Space)

#### 15.3 Checkbox and Radio
- [ ] `<input type="checkbox">` - boolean toggle
- [ ] `<input type="radio">` - single selection from group
- [ ] Checked state (`:checked` pseudo-class)
- [ ] Indeterminate state
- [ ] Radio button grouping (by `name`)
- [ ] Label association (`<label for="id">` and wrapping)
- [ ] Click on label toggles input

#### 15.4 Select Dropdowns
- [ ] `<select>` element
- [ ] `<option>` elements
- [ ] `<optgroup>` for grouping
- [ ] Single selection mode
- [ ] Multiple selection mode (`multiple` attribute)
- [ ] `size` attribute for visible rows
- [ ] Keyboard navigation (arrows, type-ahead)
- [ ] Native dropdown rendering or custom

#### 15.5 Other Input Types
- [ ] `<input type="hidden">` - hidden form data
- [ ] `<input type="file">` - file picker
- [ ] `<input type="color">` - color picker
- [ ] `<input type="date">` - date picker
- [ ] `<input type="time">` - time picker
- [ ] `<input type="datetime-local">` - datetime picker
- [ ] `<input type="range">` - slider
- [ ] `<input type="checkbox">` styled as toggle (CSS)

#### 15.6 Form Element
- [ ] `<form>` element
- [ ] `action` attribute (submission URL)
- [ ] `method` attribute (GET/POST)
- [ ] `enctype` attribute (form encoding)
- [ ] `target` attribute
- [ ] `novalidate` attribute
- [ ] Form submission on Enter in text input
- [ ] Form reset behavior

#### 15.7 Form Submission
- [ ] Collect form data (`FormData` API)
- [ ] URL encoding (`application/x-www-form-urlencoded`)
- [ ] Multipart encoding (`multipart/form-data`)
- [ ] JSON encoding (for `fetch`)
- [ ] `submit` event with `preventDefault()` support
- [ ] Submit button identification
- [ ] Form validation before submit

#### 15.8 HTML5 Validation
- [ ] `required` attribute
- [ ] `pattern` attribute (regex)
- [ ] `min`/`max` for numbers and dates
- [ ] `step` for numbers
- [ ] Type-based validation (email, url)
- [ ] `:valid` and `:invalid` pseudo-classes
- [ ] `:required` and `:optional` pseudo-classes
- [ ] `setCustomValidity()` method
- [ ] `checkValidity()` method
- [ ] `reportValidity()` method
- [ ] Validation message display

##### ValidityState API
```rust
pub struct ValidityState {
    pub value_missing: bool,      // required but empty
    pub type_mismatch: bool,      // wrong type (email, url)
    pub pattern_mismatch: bool,   // doesn't match pattern
    pub too_long: bool,           // exceeds maxlength
    pub too_short: bool,          // below minlength
    pub range_underflow: bool,    // below min
    pub range_overflow: bool,     // above max
    pub step_mismatch: bool,      // doesn't match step
    pub bad_input: bool,          // unparseable input
    pub custom_error: bool,       // setCustomValidity called
    pub valid: bool,              // all checks pass
}
```

#### 15.9 Fieldset and Legend
- [ ] `<fieldset>` grouping
- [ ] `<legend>` caption
- [ ] `disabled` on fieldset disables all descendants

#### 15.10 Datalist
- [ ] `<datalist>` for autocomplete options
- [ ] `<input list="datalist-id">` association
- [ ] Dropdown display

### Third-Party Libraries
- Consider: `chrono` for date/time handling
- Windows: Native date/time/color pickers via common controls

### Custom Implementation Required
```rust
// Text input component
pub struct TextInputState {
    value: String,
    selection_start: usize,
    selection_end: usize,
    cursor_visible: bool,
    composition: Option<CompositionState>,
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
}

pub struct FormData {
    entries: Vec<(String, FormDataValue)>,
}

pub enum FormDataValue {
    String(String),
    File(FileEntry),
}

pub struct FormValidator {
    // Validates form elements and collects errors
}
```

### Acceptance Criteria
- [ ] Text input accepts typing
- [ ] Password input masks characters
- [ ] Checkboxes toggle on click
- [ ] Radio buttons select exclusively
- [ ] Select dropdowns open and select
- [ ] Form submission collects all values
- [ ] HTML5 validation shows errors
- [ ] Tab navigates between form elements
- [ ] Enter submits form
- [ ] Pass 40% of WPT `html/semantics/forms/` tests

### Risk Assessment
- **Text editing**: Complex cursor and selection management
- **IME**: Composition for non-Latin input
- **Clipboard**: Platform-specific clipboard access
- **Validation UX**: Error message positioning and styling

---

## Phase 16: Images & Media

### Overview
Implement image loading, decoding, and display. Images are essential for any website.

### Priority: High
### Estimated Duration: 3-4 weeks
### Dependencies: Phase 14 (Events)

### Sub-Tasks

#### 16.1 Image Element
- [ ] `<img>` element
- [ ] `src` attribute (URL loading)
- [ ] `alt` attribute (accessibility)
- [ ] `width` and `height` attributes
- [ ] `srcset` and `sizes` for responsive images
- [ ] `loading` attribute (`lazy`, `eager`)
- [ ] `decoding` attribute (`async`, `sync`, `auto`)
- [ ] `crossorigin` attribute
- [ ] `ismap` and `usemap` (image maps, low priority)

#### 16.2 Image Decoding
- [ ] PNG format
- [ ] JPEG format
- [ ] GIF format (static and animated)
- [ ] WebP format
- [ ] AVIF format (optional)
- [ ] SVG format (ties to Phase 23)
- [ ] ICO format (favicons)
- [ ] BMP format

#### 16.3 Image Loading
- [ ] Async image fetch with progress
- [ ] Decode off main thread
- [ ] Image cache (memory + disk)
- [ ] Cache headers (max-age, etag)
- [ ] Error handling (`onerror` event)
- [ ] Load event (`onload`)
- [ ] `naturalWidth` and `naturalHeight`
- [ ] `complete` property

#### 16.4 Image Display
- [ ] Render decoded images to texture
- [ ] `object-fit` property (`contain`, `cover`, `fill`, `none`, `scale-down`)
- [ ] `object-position` property
- [ ] Aspect ratio preservation
- [ ] Image interpolation quality

#### 16.5 Background Images
- [ ] `background-image: url()`
- [ ] Multiple backgrounds
- [ ] `background-size` (`auto`, `cover`, `contain`, length)
- [ ] `background-position`
- [ ] `background-repeat` (`repeat`, `no-repeat`, `repeat-x`, `repeat-y`)
- [ ] `background-origin`
- [ ] `background-clip`
- [ ] `background-attachment` (`scroll`, `fixed`, `local`)
- [ ] `background` shorthand

#### 16.6 Image Replacement
- [ ] `<picture>` element
- [ ] `<source>` with media queries
- [ ] Art direction (different crops for different sizes)
- [ ] Format selection (WebP with JPEG fallback)

#### 16.7 Favicons
- [ ] `<link rel="icon">` parsing
- [ ] Multiple sizes support
- [ ] ICO and PNG formats
- [ ] Pass favicon to HiWave chrome

#### 16.8 Image APIs
- [ ] `Image()` constructor
- [ ] `decode()` promise
- [ ] Canvas `drawImage()` integration

#### 16.9 Lazy Loading
- [ ] Intersection Observer based loading
- [ ] `loading="lazy"` attribute
- [ ] Placeholder during load
- [ ] Root margin for preloading

### Third-Party Libraries
- **Recommended:** `image` crate - Pure Rust image decoding
  - Supports PNG, JPEG, GIF, WebP, BMP, ICO
  - AVIF via `image` features or separate crate
- `gif` crate for animated GIF frame extraction
- Consider: `fast_image_resize` for resizing

### Custom Implementation Required
```rust
// New module: rustkit-image or extend rustkit-compositor
pub struct ImageCache {
    memory_cache: LruCache<Url, CachedImage>,
    disk_cache: Option<DiskCache>,
    pending_loads: HashMap<Url, PendingLoad>,
}

pub struct CachedImage {
    url: Url,
    data: ImageData,
    natural_size: (u32, u32),
    texture: Option<wgpu::Texture>,
    decoded_at: Instant,
}

pub enum ImageData {
    Static(RgbaImage),
    Animated(AnimatedImage),
}

pub struct AnimatedImage {
    frames: Vec<AnimationFrame>,
    loop_count: u32,  // 0 = infinite
}

pub struct AnimationFrame {
    image: RgbaImage,
    delay_ms: u32,
}

// Image loading
pub async fn load_image(url: &Url) -> Result<CachedImage, ImageError> {
    let bytes = fetch(url).await?;
    let format = image::guess_format(&bytes)?;
    let decoded = image::load_from_memory(&bytes)?;
    Ok(CachedImage::from_dynamic_image(decoded))
}
```

### Acceptance Criteria
- [ ] PNG images display correctly
- [ ] JPEG images display correctly
- [ ] Animated GIFs animate
- [ ] WebP images work
- [ ] Background images display
- [ ] `object-fit: cover` works
- [ ] Lazy loading defers offscreen images
- [ ] Image errors show alt text or broken image
- [ ] Images scale to fit layout
- [ ] Favicons appear in browser chrome
- [ ] Pass 40% of WPT `html/semantics/embedded-content/the-img-element/` tests

### Risk Assessment
- **Memory**: Large images consume significant memory
- **Decoding time**: Large images can block rendering
- **Animated images**: Frame timing and memory for long GIFs
- **Cache invalidation**: Stale image issues

---

## Phase 17: CSS Flexbox

### Overview
Implement CSS Flexbox, the most commonly used layout system for one-dimensional layouts (rows or columns).

### Priority: High
### Estimated Duration: 3-4 weeks
### Dependencies: Phase 12 (Box Model)

### Sub-Tasks

#### 17.1 Flex Container
- [ ] `display: flex` and `display: inline-flex`
- [ ] `flex-direction` (`row`, `row-reverse`, `column`, `column-reverse`)
- [ ] `flex-wrap` (`nowrap`, `wrap`, `wrap-reverse`)
- [ ] `flex-flow` shorthand
- [ ] `justify-content` (`flex-start`, `flex-end`, `center`, `space-between`, `space-around`, `space-evenly`)
- [ ] `align-items` (`stretch`, `flex-start`, `flex-end`, `center`, `baseline`)
- [ ] `align-content` (for wrapped lines)
- [ ] `gap`, `row-gap`, `column-gap`

#### 17.2 Flex Items
- [ ] `order` property
- [ ] `flex-grow`
- [ ] `flex-shrink`
- [ ] `flex-basis`
- [ ] `flex` shorthand
- [ ] `align-self` (override container's `align-items`)

#### 17.3 Flexbox Layout Algorithm
The flexbox algorithm is multi-step:

```rust
pub fn layout_flex_container(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
) {
    // 1. Determine main axis and cross axis
    let (main_axis, cross_axis) = match container.flex_direction {
        FlexDirection::Row | FlexDirection::RowReverse => (Axis::Horizontal, Axis::Vertical),
        FlexDirection::Column | FlexDirection::ColumnReverse => (Axis::Vertical, Axis::Horizontal),
    };

    // 2. Collect flex items (skip absolutely positioned)
    let items: Vec<_> = container.children
        .iter_mut()
        .filter(|c| c.position != Position::Absolute && c.position != Position::Fixed)
        .collect();

    // 3. Determine available main and cross size
    let available_main = /* ... */;
    let available_cross = /* ... */;

    // 4. Calculate flex base sizes and hypothetical main sizes
    for item in &mut items {
        item.flex_base_size = calculate_flex_base_size(item, available_main);
        item.hypothetical_main_size = clamp(
            item.flex_base_size,
            item.min_main_size,
            item.max_main_size,
        );
    }

    // 5. Collect into flex lines (if wrapping)
    let lines = collect_flex_lines(&items, available_main, container.flex_wrap);

    // 6. Resolve flexible lengths (grow/shrink)
    for line in &mut lines {
        resolve_flexible_lengths(line, available_main);
    }

    // 7. Calculate cross sizes
    for line in &mut lines {
        calculate_cross_sizes(line, available_cross, container.align_items);
    }

    // 8. Main axis alignment (justify-content)
    for line in &mut lines {
        distribute_main_axis(line, available_main, container.justify_content);
    }

    // 9. Cross axis alignment (align-items, align-self)
    for line in &mut lines {
        align_cross_axis(line, container.align_items);
    }

    // 10. Multi-line alignment (align-content)
    if lines.len() > 1 {
        align_content(&mut lines, available_cross, container.align_content);
    }

    // 11. Handle reverse directions
    if container.flex_direction.is_reverse() {
        reverse_main_positions(&mut items);
    }
}
```

#### 17.4 Min/Max Constraints
- [ ] `min-width`, `min-height` on flex items
- [ ] `max-width`, `max-height` on flex items
- [ ] Automatic minimum size (content-based)
- [ ] `min-content` and `max-content` sizing

#### 17.5 Aspect Ratio
- [ ] `aspect-ratio` property
- [ ] Interaction with flex sizing

### Third-Party Libraries
- None required - must be custom implementation
- Reference: [CSS Flexbox Specification](https://www.w3.org/TR/css-flexbox-1/)

### Custom Implementation Required
```rust
// Extend rustkit-layout
pub struct FlexContainer {
    direction: FlexDirection,
    wrap: FlexWrap,
    justify_content: JustifyContent,
    align_items: AlignItems,
    align_content: AlignContent,
    gap: (Length, Length),
}

pub struct FlexItem {
    order: i32,
    flex_grow: f32,
    flex_shrink: f32,
    flex_basis: FlexBasis,
    align_self: Option<AlignSelf>,

    // Computed during layout
    flex_base_size: f32,
    hypothetical_main_size: f32,
    target_main_size: f32,
    outer_main_size: f32,
    cross_size: f32,
    main_position: f32,
    cross_position: f32,
}

pub enum FlexBasis {
    Auto,
    Content,
    Length(Length),
}
```

### Acceptance Criteria
- [ ] `display: flex` creates flex container
- [ ] `flex-direction` changes layout axis
- [ ] `flex-wrap` creates multiple lines
- [ ] `justify-content: center` centers items
- [ ] `align-items: center` centers cross-axis
- [ ] `flex-grow` distributes extra space
- [ ] `flex-shrink` shrinks items proportionally
- [ ] `gap` adds spacing between items
- [ ] Nested flex containers work
- [ ] Pass 60% of WPT `css/css-flexbox/` tests

### Risk Assessment
- **Algorithm complexity**: Flexbox algorithm has many steps and edge cases
- **Min/max constraints**: Interaction with flex sizing is tricky
- **Performance**: Flexbox layout can require multiple passes

---

## Phase 18: Scrolling & Overflow

### Overview
Implement scrolling containers, overflow handling, and scroll-related APIs.

### Priority: High
### Estimated Duration: 3-4 weeks
### Dependencies: Phase 14 (Events)

### Sub-Tasks

#### 18.1 Overflow Properties
- [ ] `overflow-x` and `overflow-y`
- [ ] `overflow` shorthand
- [ ] Values: `visible`, `hidden`, `scroll`, `auto`, `clip`
- [ ] Scroll container detection

#### 18.2 Scrollbar Rendering
- [ ] Native scrollbar appearance
- [ ] Custom scrollbar styling (`::-webkit-scrollbar`, limited)
- [ ] `scrollbar-width` (`auto`, `thin`, `none`)
- [ ] `scrollbar-color`
- [ ] Scrollbar gutter handling (`scrollbar-gutter`)

#### 18.3 Scroll Events
- [ ] `scroll` event
- [ ] `scrollend` event (when scrolling stops)
- [ ] Throttling for performance

#### 18.4 Wheel Events
- [ ] `wheel` event
- [ ] `deltaX`, `deltaY`, `deltaZ`
- [ ] `deltaMode` (pixels, lines, pages)
- [ ] Passive wheel listeners for performance

#### 18.5 Scroll APIs
- [ ] `element.scrollTop`, `element.scrollLeft` (get/set)
- [ ] `element.scrollWidth`, `element.scrollHeight`
- [ ] `element.clientWidth`, `element.clientHeight`
- [ ] `element.scroll()`, `element.scrollTo()`
- [ ] `element.scrollBy()`
- [ ] `element.scrollIntoView()`
- [ ] `window.scrollX`, `window.scrollY`
- [ ] `window.scroll()`, `window.scrollTo()`, `window.scrollBy()`

#### 18.6 Smooth Scrolling
- [ ] `scroll-behavior: smooth`
- [ ] `behavior: 'smooth'` in scroll methods
- [ ] Animation easing
- [ ] Interruption handling

#### 18.7 Scroll Snap
- [ ] `scroll-snap-type`
- [ ] `scroll-snap-align`
- [ ] `scroll-snap-stop`
- [ ] Snap point calculation

#### 18.8 Overscroll Behavior
- [ ] `overscroll-behavior`
- [ ] Bounce effects (platform-specific)
- [ ] Scroll chaining prevention

#### 18.9 Position: Sticky
- [ ] Sticky positioning within scroll container
- [ ] Sticky constraints (`top`, `bottom`, `left`, `right`)
- [ ] Multiple sticky elements
- [ ] Sticky with overflow

#### 18.10 Scroll Restoration
- [ ] Remember scroll position on navigation
- [ ] `history.scrollRestoration`

### Third-Party Libraries
- None required - custom implementation
- May use platform scrollbar rendering

### Custom Implementation Required
```rust
// Scroll container state
pub struct ScrollState {
    scroll_x: f32,
    scroll_y: f32,
    scroll_width: f32,
    scroll_height: f32,
    viewport_width: f32,
    viewport_height: f32,
    momentum: Option<ScrollMomentum>,
    snap_targets: Vec<SnapTarget>,
}

pub struct ScrollMomentum {
    velocity_x: f32,
    velocity_y: f32,
    deceleration: f32,
}

// Scroll container detection
pub fn is_scroll_container(style: &ComputedStyle) -> bool {
    matches!(
        style.overflow_x,
        Overflow::Scroll | Overflow::Auto
    ) || matches!(
        style.overflow_y,
        Overflow::Scroll | Overflow::Auto
    )
}

// Scrollbar rendering
pub struct Scrollbar {
    orientation: ScrollbarOrientation,
    track_rect: Rect,
    thumb_rect: Rect,
    hover_state: ScrollbarHoverState,
}
```

### Acceptance Criteria
- [ ] `overflow: scroll` shows scrollbars
- [ ] `overflow: hidden` clips content
- [ ] Mouse wheel scrolls content
- [ ] Scrollbar drag works
- [ ] `scrollTop`/`scrollLeft` work
- [ ] `scrollIntoView()` scrolls to element
- [ ] `scroll-behavior: smooth` animates
- [ ] Scroll events fire
- [ ] `position: sticky` works
- [ ] Nested scroll containers work
- [ ] Pass 40% of WPT `css/css-overflow/` tests

### Risk Assessment
- **Performance**: Scrolling must be 60fps
- **Hit testing**: Scrolled content coordinates
- **Sticky**: Complex interaction with overflow
- **Touch scrolling**: Inertia and momentum

---

## Phase 19: Navigation & History

### Overview
Implement browser navigation, the History API, and page lifecycle events.

### Priority: High
### Estimated Duration: 3-4 weeks
### Dependencies: Phase 15 (Forms), Phase 16 (Images), Phase 18 (Scrolling)

### Sub-Tasks

#### 19.1 Navigation Types
- [ ] Link clicks (`<a href>`)
- [ ] Form submissions
- [ ] `location.href` assignment
- [ ] `location.assign()`
- [ ] `location.replace()`
- [ ] `location.reload()`
- [ ] `window.open()` (new window/tab)

#### 19.2 History API
- [ ] `history.length`
- [ ] `history.state`
- [ ] `history.pushState(state, title, url)`
- [ ] `history.replaceState(state, title, url)`
- [ ] `history.back()`
- [ ] `history.forward()`
- [ ] `history.go(delta)`
- [ ] `popstate` event

#### 19.3 Location Object
- [x] `location.href`, `protocol`, `host`, `hostname`, `port`
- [x] `location.pathname`, `search`, `hash`, `origin`
- [ ] `location.assign(url)`
- [ ] `location.replace(url)`
- [ ] `location.reload()`

#### 19.4 Hash Navigation
- [ ] `#fragment` links scroll to element
- [ ] `hashchange` event
- [ ] `location.hash` updates

#### 19.5 Page Lifecycle Events
- [ ] `DOMContentLoaded` - DOM parsed
- [ ] `load` - all resources loaded
- [ ] `beforeunload` - leaving page (with prompt)
- [ ] `unload` - page unloading
- [ ] `pagehide` / `pageshow` - visibility changes
- [ ] `visibilitychange` - tab visibility

#### 19.6 Navigation Timing
- [ ] `performance.timing` (deprecated but common)
- [ ] `PerformanceNavigationTiming` API
- [ ] Navigation start, response start, DOM complete, load complete

#### 19.7 Navigation Interception
- [ ] `beforeunload` can cancel navigation
- [ ] Form validation before submission
- [ ] Navigation API (modern, optional)

### Third-Party Libraries
- None required

### Custom Implementation Required
```rust
// Navigation state machine
pub struct Navigator {
    history: NavigationHistory,
    current_entry: HistoryEntry,
    pending_navigation: Option<PendingNavigation>,
}

pub struct NavigationHistory {
    entries: Vec<HistoryEntry>,
    current_index: usize,
}

pub struct HistoryEntry {
    url: Url,
    state: Option<JsValue>,
    scroll_position: (f32, f32),
    title: String,
}

pub enum NavigationType {
    Navigate,      // New navigation
    Reload,        // Same page reload
    BackForward,   // History navigation
    Replace,       // Replace current entry
}

// Page lifecycle
pub enum PageLifecycle {
    Initial,
    Loading,
    Interactive,   // DOM ready
    Complete,      // All resources loaded
    Unloading,
}
```

### Acceptance Criteria
- [ ] Link clicks navigate to new page
- [ ] `history.pushState` updates URL without reload
- [ ] Back/forward buttons work
- [ ] `popstate` fires on history navigation
- [ ] Hash links scroll to element
- [ ] `beforeunload` can show confirmation
- [ ] `DOMContentLoaded` fires at right time
- [ ] Form submission navigates with data
- [ ] `location.reload()` refreshes page
- [ ] Scroll position restored on back

### Risk Assessment
- **SPA complexity**: History API edge cases
- **Unload handling**: Data loss prevention
- **Timing**: Event order is specified precisely

---

## Phase 20: Security & Isolation

### Overview
Implement web security features including same-origin policy, CSP, and CORS.

### Priority: Critical
### Estimated Duration: 4-5 weeks
### Dependencies: Phase 19 (Navigation)

### Sub-Tasks

#### 20.1 Origin Model
- [ ] Origin definition (scheme + host + port)
- [ ] Same-origin checks
- [ ] `document.domain` (legacy, limited support)
- [ ] Opaque origins (data:, file:, etc.)

#### 20.2 Same-Origin Policy
- [ ] Script access restrictions
- [ ] DOM access restrictions
- [ ] Cookie access restrictions
- [ ] Storage access restrictions

#### 20.3 Content Security Policy (CSP)
- [ ] CSP header parsing (`Content-Security-Policy`)
- [ ] `script-src` directive
- [ ] `style-src` directive
- [ ] `img-src` directive
- [ ] `connect-src` directive (XHR/fetch)
- [ ] `font-src` directive
- [ ] `frame-src` / `child-src` directive
- [ ] `default-src` directive
- [ ] `base-uri` directive
- [ ] `form-action` directive
- [ ] `frame-ancestors` directive
- [ ] Nonce support (`'nonce-xxx'`)
- [ ] Hash support (`'sha256-xxx'`)
- [ ] `'unsafe-inline'` and `'unsafe-eval'`
- [ ] `report-uri` / `report-to` (optional)

##### CSP Enforcement
```rust
pub struct ContentSecurityPolicy {
    directives: HashMap<CspDirective, Vec<CspSource>>,
}

pub enum CspDirective {
    DefaultSrc,
    ScriptSrc,
    StyleSrc,
    ImgSrc,
    ConnectSrc,
    FontSrc,
    FrameSrc,
    // ...
}

pub enum CspSource {
    Self_,
    None,
    UnsafeInline,
    UnsafeEval,
    Host(String),
    Scheme(String),
    Nonce(String),
    Hash(HashAlgorithm, Vec<u8>),
}

impl ContentSecurityPolicy {
    pub fn allows_script(&self, source: &ScriptSource) -> bool {
        // Check against script-src or default-src
    }

    pub fn allows_style(&self, source: &StyleSource) -> bool {
        // Check against style-src or default-src
    }
}
```

#### 20.4 Cross-Origin Resource Sharing (CORS)
- [ ] Simple requests (GET, HEAD, POST with simple headers)
- [ ] Preflight requests (OPTIONS)
- [ ] `Access-Control-Allow-Origin` header
- [ ] `Access-Control-Allow-Methods` header
- [ ] `Access-Control-Allow-Headers` header
- [ ] `Access-Control-Allow-Credentials` header
- [ ] `Access-Control-Expose-Headers` header
- [ ] `Access-Control-Max-Age` header
- [ ] Credentialed requests
- [ ] Wildcard restrictions

##### CORS Flow
```rust
pub struct CorsChecker {
    // For simple requests:
    // 1. Add Origin header to request
    // 2. Check Access-Control-Allow-Origin in response
    // 3. Block if not matching or wildcard with credentials

    // For preflighted requests:
    // 1. Send OPTIONS with Access-Control-Request-* headers
    // 2. Check allowed methods/headers in response
    // 3. Cache preflight result
    // 4. Send actual request if allowed
}
```

#### 20.5 Secure Contexts
- [ ] HTTPS detection
- [ ] `window.isSecureContext`
- [ ] Restrict APIs to secure contexts (geolocation, etc.)
- [ ] Mixed content blocking
- [ ] `upgrade-insecure-requests`

#### 20.6 Cookie Security
- [ ] `Secure` attribute (HTTPS only)
- [ ] `HttpOnly` attribute (no JS access)
- [ ] `SameSite` attribute (`Strict`, `Lax`, `None`)
- [ ] Cookie scope (domain, path)
- [ ] Third-party cookie handling

#### 20.7 Referrer Policy
- [ ] `Referrer-Policy` header
- [ ] `referrerpolicy` attribute
- [ ] Policies: `no-referrer`, `origin`, `strict-origin-when-cross-origin`, etc.

#### 20.8 Feature Policy / Permissions Policy
- [ ] `Permissions-Policy` header
- [ ] Feature restrictions (camera, microphone, geolocation, etc.)

#### 20.9 Sandboxing (Future)
- [ ] Process isolation (separate processes per origin)
- [ ] `<iframe sandbox>` attribute
- [ ] Sandbox flags

### Third-Party Libraries
- None required for core security
- Consider: `cookie` crate for cookie parsing

### Custom Implementation Required
```rust
// Security context
pub struct SecurityContext {
    origin: Origin,
    csp: Option<ContentSecurityPolicy>,
    referrer_policy: ReferrerPolicy,
    is_secure_context: bool,
}

pub struct Origin {
    scheme: String,
    host: String,
    port: Option<u16>,
}

impl Origin {
    pub fn same_origin(&self, other: &Origin) -> bool {
        self.scheme == other.scheme
            && self.host == other.host
            && self.port == other.port
    }
}

// CORS check
pub fn check_cors(
    request: &Request,
    response: &Response,
    credentials: bool,
) -> Result<(), CorsError> {
    let origin = request.headers.get("Origin");
    let allow_origin = response.headers.get("Access-Control-Allow-Origin");

    match (origin, allow_origin) {
        (Some(o), Some(a)) if a == "*" && !credentials => Ok(()),
        (Some(o), Some(a)) if a == o => Ok(()),
        _ => Err(CorsError::NotAllowed),
    }
}
```

### Acceptance Criteria
- [ ] Cross-origin XHR blocked by default
- [ ] CORS preflight sent for complex requests
- [ ] CORS allowed when headers match
- [ ] CSP blocks inline scripts when configured
- [ ] CSP allows scripts with correct nonce
- [ ] Same-origin policy prevents DOM access
- [ ] Secure cookies not sent over HTTP
- [ ] `SameSite` cookies respected
- [ ] Mixed content blocked on HTTPS pages
- [ ] Pass 40% of WPT `cors/` tests

### Risk Assessment
- **Security critical**: Bugs here can expose user data
- **Complexity**: Many edge cases in specs
- **Compatibility**: Sites depend on exact behavior
- **Testing**: Hard to test all security scenarios

---

## Summary Table

| Phase | Name | Duration | Complexity | Dependencies |
|-------|------|----------|------------|--------------|
| 14 | Event Handling | 4-5 weeks | High | 12, 13 |
| 15 | Forms & Input | 4-5 weeks | High | 14 |
| 16 | Images & Media | 3-4 weeks | Medium | 14 |
| 17 | CSS Flexbox | 3-4 weeks | High | 12 |
| 18 | Scrolling & Overflow | 3-4 weeks | Medium-High | 14 |
| 19 | Navigation & History | 3-4 weeks | Medium | 15, 16, 18 |
| 20 | Security & Isolation | 4-5 weeks | High | 19 |

**Total Estimated Duration:** 24-31 weeks (~6-8 months)

---

## Recommended Order

Given dependencies and importance:

1. **Phase 14: Events** - Foundation for all interactivity
2. **Phase 17: Flexbox** - Can parallelize with events work
3. **Phase 15: Forms** - After events, enables user input
4. **Phase 16: Images** - After events, enables visual content
5. **Phase 18: Scrolling** - After events, long pages need this
6. **Phase 19: Navigation** - After forms/images/scroll
7. **Phase 20: Security** - Last, builds on everything

Note: Phase 17 (Flexbox) can be developed in parallel with other phases as it's primarily layout work.

---

## Critical Path Analysis

The minimal path to a "usable" browser:

```
Phase 14 (Events)
    ↓
Phase 15 (Forms) ←── Enables: login, search, data entry
    ↓
Phase 16 (Images) ←── Enables: visual websites
    ↓
Phase 18 (Scrolling) ←── Enables: long pages
    ↓
Phase 20 (Security) ←── Enables: real-world deployment
```

With this path (~18-23 weeks), RustKit could render and interact with many real websites, though without Flexbox many modern layouts would break.

---

## Milestones

### M1: Interactive (End of Phase 15)
- Click events work
- Forms accept input
- Can submit a login form

### M2: Visual (End of Phase 16)
- Images display
- Background images work
- Pages look complete

### M3: Scrollable (End of Phase 18)
- Long pages scroll
- Overflow containers work
- Sticky headers work

### M4: Navigable (End of Phase 19)
- Links work
- Back/forward works
- SPAs function

### M5: Secure (End of Phase 20)
- CORS works
- CSP enforced
- Ready for real-world use
