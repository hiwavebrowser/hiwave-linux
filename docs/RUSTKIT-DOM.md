# RustKit DOM

The DOM module provides HTML parsing and DOM tree manipulation for the RustKit browser engine.

## Overview

RustKit DOM uses:
- **rustkit-html**: RustKit's own HTML5 parser (replaced html5ever)
  - Full HTML5 tokenizer with 40+ states
  - Tree builder with 23 insertion modes
  - Adoption Agency Algorithm for misnested formatting
  - Table parsing with foster parenting
  - Fragment parsing for innerHTML
- **Rc/RefCell**: Reference-counted nodes with interior mutability
- **HashMap indexing**: Fast element lookup by ID

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                       Document                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  root: Rc<Node> (Document node)                      │    │
│  │  nodes: HashMap<NodeId, Rc<Node>>                    │    │
│  │  elements_by_id: HashMap<String, Rc<Node>>           │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        DOM Tree                              │
│                                                              │
│  Document                                                    │
│  └── html                                                    │
│      ├── head                                                │
│      │   └── title                                           │
│      │       └── #text "Page Title"                          │
│      └── body                                                │
│          ├── div.container                                   │
│          │   └── p#main                                      │
│          │       └── #text "Hello"                           │
│          └── ...                                             │
└─────────────────────────────────────────────────────────────┘
```

## Usage

### Parsing HTML

```rust
use rustkit_dom::Document;

let html = r#"<!DOCTYPE html>
<html>
<head><title>My Page</title></head>
<body>
    <div id="content" class="container">
        <p>Hello, world!</p>
    </div>
</body>
</html>"#;

let doc = Document::parse_html(html)?;
```

### Accessing Elements

```rust
// Document structure
let root = doc.root();
let html = doc.document_element();
let head = doc.head();
let body = doc.body();

// Title
let title = doc.title(); // Some("My Page")

// By ID
let content = doc.get_element_by_id("content");

// By tag name
let divs = doc.get_elements_by_tag_name("div");

// By class name
let containers = doc.get_elements_by_class_name("container");
```

### Query Selectors

```rust
use rustkit_dom::QuerySelector;

// ID selector
let elem = QuerySelector::select(&doc, "#content");

// Class selector
let elems = QuerySelector::select(&doc, ".container");

// Tag selector
let paragraphs = QuerySelector::select(&doc, "p");
```

### Node Properties

```rust
let node = doc.get_element_by_id("content").unwrap();

// Node type checks
assert!(node.is_element());
assert!(!node.is_text());

// Element properties
let tag = node.tag_name(); // Some("div")
let id = node.get_attribute("id"); // Some("content")
let class = node.get_attribute("class"); // Some("container")

// Text content
let text = node.text_content(); // "Hello, world!"
```

### Tree Traversal

```rust
let node = doc.body().unwrap();

// Children
for child in node.children() {
    println!("Child: {:?}", child.tag_name());
}

// First/last child
let first = node.first_child();
let last = node.last_child();

// Siblings
let prev = node.previous_sibling();
let next = node.next_sibling();

// Parent
let parent = node.parent();

// Full document traversal
doc.traverse(|node| {
    if let Some(tag) = node.tag_name() {
        println!("Visiting: {}", tag);
    }
});
```

## Node Types

| Type | Description |
|------|-------------|
| `Document` | Root document node |
| `DocumentType` | DOCTYPE declaration |
| `Element` | HTML element with tag, attributes |
| `Text` | Text content |
| `Comment` | HTML comment |
| `ProcessingInstruction` | Processing instruction (rare) |

## Integration with CSS/Layout

The DOM provides the input for style computation:

```rust
// 1. Parse HTML to DOM
let doc = Document::parse_html(html)?;

// 2. For each element, compute styles
doc.traverse(|node| {
    if node.is_element() {
        // Get inline styles
        let style = node.get_attribute("style");
        
        // Get class for matching
        let class = node.get_attribute("class");
        
        // Compute final styles...
    }
});

// 3. Build layout tree from styled DOM
```

## Performance Considerations

1. **Rc/RefCell overhead**: Each node is heap-allocated
2. **HashMap lookups**: O(1) for ID lookups
3. **Tree traversal**: O(n) for full document
4. **Memory**: ~200 bytes per node typical

## Testing

```bash
# Run DOM tests
cargo test -p rustkit-dom

# With logging
RUST_LOG=rustkit_dom=debug cargo test -p rustkit-dom
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: html-dom-pipeline*

