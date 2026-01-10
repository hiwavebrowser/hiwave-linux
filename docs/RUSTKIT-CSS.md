# RustKit CSS

The CSS module provides stylesheet parsing, cascade/specificity, and computed style generation for the RustKit browser engine.

## Overview

RustKit CSS provides:
- **Property parsing**: Parse CSS values (colors, lengths, etc.)
- **Stylesheet parsing**: Parse complete CSS files via `rustkit-cssparser`
- **Cascade**: Apply rules based on specificity and origin
- **Inheritance**: Propagate inherited properties to children

### Dependencies

| Crate | Purpose |
|-------|---------|
| `rustkit-cssparser` | CSS tokenizer and value parser (replaced external `cssparser`) |
| `selectors` | CSS selector matching |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Style System                            │
│                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │
│  │ Stylesheet  │───▶│   Cascade   │───▶│ Computed    │      │
│  │   Parser    │    │  Resolver   │    │   Styles    │      │
│  └─────────────┘    └─────────────┘    └─────────────┘      │
│         │                  │                  │              │
│         ▼                  ▼                  ▼              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │
│  │    Rules    │    │ Specificity │    │ Per-Element │      │
│  │ Declarations│    │   Origin    │    │   Values    │      │
│  └─────────────┘    └─────────────┘    └─────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

## Usage

### Parsing Colors

```rust
use rustkit_css::parse_color;

let red = parse_color("#ff0000");        // Some(Color { r: 255, g: 0, b: 0 })
let blue = parse_color("rgb(0, 0, 255)"); // Some(Color { r: 0, g: 0, b: 255 })
let named = parse_color("red");           // Some(Color { r: 255, g: 0, b: 0 })
let trans = parse_color("transparent");   // Some(Color::TRANSPARENT)
```

### Parsing Lengths

```rust
use rustkit_css::{parse_length, Length};

let px = parse_length("10px");     // Some(Length::Px(10.0))
let em = parse_length("1.5em");    // Some(Length::Em(1.5))
let pct = parse_length("50%");     // Some(Length::Percent(50.0))
let auto = parse_length("auto");   // Some(Length::Auto)
```

### Parsing Stylesheets

```rust
use rustkit_css::Stylesheet;

let css = r#"
    body {
        color: black;
        background-color: white;
    }
    
    .container {
        width: 100%;
        padding: 20px;
    }
    
    #header {
        height: 60px;
    }
"#;

let stylesheet = Stylesheet::parse(css)?;
```

### Computing Styles

```rust
use rustkit_css::ComputedStyle;

// Root element gets default styles
let root_style = ComputedStyle::new();

// Child inherits from parent
let child_style = ComputedStyle::inherit_from(&root_style);

// Inherited properties match parent
assert_eq!(child_style.color, root_style.color);
assert_eq!(child_style.font_size, root_style.font_size);

// Non-inherited properties get defaults
assert_eq!(child_style.display, Display::Block);
```

## Supported Properties

### Box Model

| Property | Type | Default |
|----------|------|---------|
| `display` | Display | block |
| `position` | Position | static |
| `width` | Length | auto |
| `height` | Length | auto |
| `margin-*` | Length | 0 |
| `padding-*` | Length | 0 |
| `border-*-width` | Length | 0 |
| `border-*-color` | Color | black |

### Typography (inherited)

| Property | Type | Default |
|----------|------|---------|
| `color` | Color | black |
| `font-size` | Length | 16px |
| `font-weight` | FontWeight | 400 |
| `font-style` | FontStyle | normal |
| `font-family` | String | sans-serif |
| `line-height` | f32 | 1.2 |
| `text-align` | TextAlign | left |

### Visual

| Property | Type | Default |
|----------|------|---------|
| `background-color` | Color | transparent |
| `opacity` | f32 | 1.0 |
| `overflow-x` | Overflow | visible |
| `overflow-y` | Overflow | visible |

## Length Resolution

```rust
let length = Length::Rem(2.0);
let px = length.to_px(
    font_size: 16.0,       // Current font size (for em)
    root_font_size: 16.0,  // Root font size (for rem)
    container_size: 800.0, // Container width (for %)
);
// px = 32.0
```

## Inheritance

**Inherited properties** (from parent):
- `color`
- `font-size`, `font-weight`, `font-style`, `font-family`
- `line-height`
- `text-align`

**Non-inherited properties** (get defaults):
- `display`, `position`
- `width`, `height`
- `margin-*`, `padding-*`
- `border-*`
- `background-color`
- `opacity`

## Integration with DOM

```rust
use rustkit_dom::Document;
use rustkit_css::{Stylesheet, ComputedStyle};

// 1. Parse HTML and CSS
let doc = Document::parse_html(html)?;
let styles = Stylesheet::parse(css)?;

// 2. Compute styles for each element
fn compute_styles(
    node: &Rc<Node>,
    parent_style: &ComputedStyle,
    stylesheet: &Stylesheet,
) -> ComputedStyle {
    let mut style = ComputedStyle::inherit_from(parent_style);
    
    // Apply matching rules from stylesheet
    // (selector matching not shown)
    
    // Apply inline styles
    if let Some(inline) = node.get_attribute("style") {
        // Parse and apply inline declarations
    }
    
    style
}
```

## Testing

```bash
# Run CSS tests
cargo test -p rustkit-css

# With logging
RUST_LOG=rustkit_css=debug cargo test -p rustkit-css
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: css-style-system*

