# RustKit Layout

The layout module implements block and inline layout algorithms with text shaping for the RustKit browser engine.

## Overview

RustKit Layout provides:
- **Block layout**: Stack boxes vertically (CSS block flow)
- **Inline layout**: Flow content horizontally with wrapping
- **Flexbox layout**: CSS flexible box layout
- **Grid layout**: CSS Grid layout
- **Text shaping**: DirectWrite integration via `rustkit-text` (replaced `dwrote`)
- **Display list**: Generate paint commands for rendering

### Dependencies

| Crate | Purpose |
|-------|---------|
| `rustkit-text` | DirectWrite text shaping (replaced `dwrote`) |
| `rustkit-css` | Computed styles |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Layout Pipeline                           │
│                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │
│  │  Styled     │───▶│   Layout    │───▶│  Display    │      │
│  │    DOM      │    │    Tree     │    │    List     │      │
│  └─────────────┘    └─────────────┘    └─────────────┘      │
│                            │                  │              │
│                            ▼                  ▼              │
│                     ┌─────────────┐    ┌─────────────┐      │
│                     │ Dimensions  │    │   Paint     │      │
│                     │  (boxes)    │    │  Commands   │      │
│                     └─────────────┘    └─────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

## Box Model

```
┌───────────────────────────────────────────────┐
│                    Margin                      │
│  ┌─────────────────────────────────────────┐  │
│  │                 Border                   │  │
│  │  ┌───────────────────────────────────┐  │  │
│  │  │              Padding               │  │  │
│  │  │  ┌─────────────────────────────┐  │  │  │
│  │  │  │          Content            │  │  │  │
│  │  │  │                             │  │  │  │
│  │  │  └─────────────────────────────┘  │  │  │
│  │  │                                    │  │  │
│  │  └────────────────────────────────────┘  │  │
│  │                                           │  │
│  └───────────────────────────────────────────┘  │
│                                                  │
└──────────────────────────────────────────────────┘
```

## Usage

### Creating Layout Boxes

```rust
use rustkit_layout::{LayoutBox, BoxType};
use rustkit_css::ComputedStyle;

// Create a block layout box
let style = ComputedStyle::new();
let mut layout_box = LayoutBox::new(BoxType::Block, style);

// Add children
let child_style = ComputedStyle::inherit_from(&layout_box.style);
layout_box.children.push(LayoutBox::new(BoxType::Block, child_style));
```

### Performing Layout

```rust
use rustkit_layout::{Dimensions, Rect};

// Define the containing block (viewport)
let mut containing_block = Dimensions::default();
containing_block.content = Rect::new(0.0, 0.0, 800.0, 600.0);

// Layout the box tree
layout_box.layout(&containing_block);

// Access computed dimensions
let content = layout_box.dimensions.content;
let border_box = layout_box.dimensions.border_box();
```

### Generating Display List

```rust
use rustkit_layout::{DisplayList, DisplayCommand};

// Build display list from layout tree
let display_list = DisplayList::build(&layout_box);

// Render commands
for cmd in &display_list.commands {
    match cmd {
        DisplayCommand::SolidColor(color, rect) => {
            // Fill rectangle with color
        }
        DisplayCommand::Border { color, rect, .. } => {
            // Draw border
        }
        DisplayCommand::Text { text, x, y, color, font_size } => {
            // Draw text
        }
    }
}
```

### Text Measurement

```rust
use rustkit_layout::measure_text;

let metrics = measure_text("Hello, World!", "Segoe UI", 16.0);
println!("Width: {}, Height: {}", metrics.width, metrics.height);
```

## Dimensions API

```rust
use rustkit_layout::{Dimensions, Rect, EdgeSizes};

let mut dims = Dimensions::default();

// Set content
dims.content = Rect::new(100.0, 100.0, 200.0, 100.0);

// Set padding
dims.padding = EdgeSizes {
    top: 10.0,
    right: 15.0,
    bottom: 10.0,
    left: 15.0,
};

// Set border
dims.border = EdgeSizes {
    top: 1.0,
    right: 1.0,
    bottom: 1.0,
    left: 1.0,
};

// Set margin
dims.margin = EdgeSizes {
    top: 20.0,
    right: 0.0,
    bottom: 20.0,
    left: 0.0,
};

// Get box dimensions
let padding_box = dims.padding_box();   // Content + padding
let border_box = dims.border_box();     // Content + padding + border
let margin_box = dims.margin_box();     // Full box
```

## Block Layout Algorithm

1. **Calculate width**: Fill containing block width minus margins/padding/border
2. **Position box**: Place at current cursor Y position
3. **Layout children**: Recursively layout children, stacking vertically
4. **Calculate height**: Sum of children heights (or explicit height)

```rust
fn layout_block(&mut self, containing_block: &Dimensions) {
    self.calculate_block_width(containing_block);
    self.calculate_block_position(containing_block);
    self.layout_block_children();
    self.calculate_block_height();
}
```

## Display Commands

| Command | Description |
|---------|-------------|
| `SolidColor(color, rect)` | Fill rectangle with solid color |
| `Border { color, rect, ... }` | Draw border around rectangle |
| `Text { text, x, y, ... }` | Draw text at position |

## Text Shaping

Uses DirectWrite for Windows:
- Accurate text width measurement
- Font metrics (ascent, descent)
- Font fallback handling

```rust
use rustkit_layout::measure_text;

let metrics = measure_text(
    "Hello",
    "Segoe UI",  // Font family
    16.0,        // Font size in pixels
);

// Use metrics for layout
let line_height = metrics.ascent + metrics.descent;
```

## Integration with Compositor

```rust
use rustkit_compositor::Compositor;
use rustkit_layout::{DisplayList, DisplayCommand};

// Build display list
let display_list = DisplayList::build(&root_box);

// Render to compositor surface
for cmd in &display_list.commands {
    match cmd {
        DisplayCommand::SolidColor(color, rect) => {
            // Use wgpu to fill rect
        }
        DisplayCommand::Text { text, x, y, .. } => {
            // Use DirectWrite to render text
        }
        _ => {}
    }
}
```

## Performance Considerations

1. **Incremental layout**: Only re-layout dirty subtrees
2. **Display list caching**: Cache paint commands
3. **Text shaping cache**: Cache glyph runs
4. **Box pooling**: Reuse layout box allocations

## Testing

```bash
# Run layout tests
cargo test -p rustkit-layout

# With logging
RUST_LOG=rustkit_layout=debug cargo test -p rustkit-layout
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: layout-block-inline*

