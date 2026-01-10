# Critical Gap: Display List Renderer

**Priority:** P0 - BLOCKING
**Status:** ✅ IMPLEMENTED
**Estimated Effort:** 1,500-2,000 lines (Actual: ~1,600 lines)
**Blocking:** All visual output (Canvas, SVG, WebGL, everything)

---

## The Problem

```
HTML → DOM → CSS → Layout → DisplayList → ❌ NOTHING RENDERED

Current render() function (rustkit-engine/src/lib.rs:507):

    fn render(&mut self, id: EngineViewId) -> Result<(), EngineError> {
        // For now, just render background
        // Full rendering would iterate display list
        self.compositor.render_solid_color(view.viewhost_id, self.config.background_color)
    }
```

**30,000+ lines of parsing, styling, and layout code produce a DisplayList that is never executed.**

---

## What Exists

### DisplayCommand Variants (rustkit-layout/src/lib.rs)

```rust
pub enum DisplayCommand {
    SolidColor(Color, Rect),
    Border { color, rect, widths, radii },
    Text { text, x, y, font_size, color, font_family, ... },
    TextDecoration { x, y, width, thickness, color, style },
    Image { url, src_rect, dest_rect, object_fit, opacity },
    BackgroundImage { url, rect, size, position, repeat },
    PushClip(Rect),
    PopClip,
    PushStackingContext { z_index, rect },
    PopStackingContext,

    // SVG commands
    FillRect { rect, color },
    StrokeRect { rect, color, width },
    FillCircle { cx, cy, radius, color },
    StrokeCircle { cx, cy, radius, color, width },
    FillEllipse { rect, color },
    Line { x1, y1, x2, y2, color, width },
    Polyline { points, color, width },
    FillPolygon { points, color },
    StrokePolygon { points, color, width },
}
```

### DisplayList (rustkit-layout/src/lib.rs)

```rust
pub struct DisplayList {
    pub commands: Vec<DisplayCommand>,
}

impl DisplayList {
    pub fn build(root: &LayoutBox) -> Self { ... }
}
```

### Compositor (rustkit-compositor/src/lib.rs)

```rust
impl Compositor {
    // ONLY renders solid color backgrounds
    pub fn render_solid_color(&self, view_id: ViewId, color: [f32; 4]) -> Result<()>
}
```

---

## What's Missing

### rustkit-renderer crate

A GPU renderer that:
1. Takes a `DisplayList`
2. Executes each `DisplayCommand`
3. Produces GPU draw calls via wgpu
4. Handles text rendering via DirectWrite
5. Manages textures for images

---

## Implementation Plan

### Phase 1: Core Infrastructure (~500 lines)

```
crates/rustkit-renderer/
├── Cargo.toml
└── src/
    ├── lib.rs          # Main renderer struct
    ├── pipeline.rs     # wgpu render pipelines
    ├── vertex.rs       # Vertex types and buffers
    └── batch.rs        # Draw call batching
```

#### 1.1 Vertex Types

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureVertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],  // For tinting
}
```

#### 1.2 Render Pipelines

```rust
pub struct RenderPipelines {
    pub color_pipeline: wgpu::RenderPipeline,      // Solid colors, borders
    pub texture_pipeline: wgpu::RenderPipeline,    // Images
    pub text_pipeline: wgpu::RenderPipeline,       // Glyph atlases
}
```

#### 1.3 Renderer Core

```rust
pub struct Renderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipelines: RenderPipelines,

    // Batching
    color_vertices: Vec<ColorVertex>,
    color_indices: Vec<u16>,
    texture_vertices: Vec<TextureVertex>,
    texture_indices: Vec<u16>,

    // State
    clip_stack: Vec<Rect>,
    transform_stack: Vec<Transform2D>,

    // Resources
    texture_cache: HashMap<String, wgpu::Texture>,
    glyph_cache: GlyphCache,
}
```

### Phase 2: Basic Rendering (~400 lines)

#### 2.1 Solid Color Rectangles

```rust
impl Renderer {
    fn draw_solid_rect(&mut self, rect: Rect, color: Color) {
        let c = color.to_array();
        let base = self.color_vertices.len() as u16;

        self.color_vertices.extend_from_slice(&[
            ColorVertex { position: [rect.x, rect.y], color: c },
            ColorVertex { position: [rect.right(), rect.y], color: c },
            ColorVertex { position: [rect.right(), rect.bottom()], color: c },
            ColorVertex { position: [rect.x, rect.bottom()], color: c },
        ]);

        self.color_indices.extend_from_slice(&[
            base, base + 1, base + 2,
            base, base + 2, base + 3,
        ]);
    }
}
```

#### 2.2 Borders

```rust
fn draw_border(&mut self, rect: Rect, color: Color, widths: EdgeSizes) {
    // Top border
    self.draw_solid_rect(Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: widths.top,
    }, color);

    // Right border
    self.draw_solid_rect(Rect {
        x: rect.right() - widths.right,
        y: rect.y + widths.top,
        width: widths.right,
        height: rect.height - widths.top - widths.bottom,
    }, color);

    // Bottom, Left...
}
```

### Phase 3: Text Rendering (~400 lines)

#### 3.1 Glyph Cache

```rust
pub struct GlyphCache {
    atlas: wgpu::Texture,
    atlas_size: u32,
    entries: HashMap<GlyphKey, GlyphEntry>,
    next_x: u32,
    next_y: u32,
    row_height: u32,
}

#[derive(Hash, Eq, PartialEq)]
pub struct GlyphKey {
    pub codepoint: char,
    pub font_family: String,
    pub font_size: u32,  // Fixed-point (size * 10)
    pub font_weight: u16,
    pub font_style: u8,
}

pub struct GlyphEntry {
    pub tex_coords: [f32; 4],  // u0, v0, u1, v1
    pub offset: [f32; 2],
    pub advance: f32,
}
```

#### 3.2 DirectWrite Integration

```rust
impl GlyphCache {
    pub fn rasterize_glyph(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: &GlyphKey,
    ) -> &GlyphEntry {
        if let Some(entry) = self.entries.get(key) {
            return entry;
        }

        // Use DirectWrite to render glyph to bitmap
        let bitmap = directwrite_render_glyph(key);

        // Upload to atlas
        let (x, y) = self.allocate_space(bitmap.width, bitmap.height);
        queue.write_texture(...);

        // Store entry
        self.entries.insert(key.clone(), GlyphEntry { ... });
        &self.entries[key]
    }
}
```

#### 3.3 Text Drawing

```rust
fn draw_text(
    &mut self,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color: Color,
    font_family: &str,
) {
    let mut cursor_x = x;

    for ch in text.chars() {
        let key = GlyphKey {
            codepoint: ch,
            font_family: font_family.to_string(),
            font_size: (font_size * 10.0) as u32,
            font_weight: 400,
            font_style: 0,
        };

        let glyph = self.glyph_cache.rasterize_glyph(&self.device, &self.queue, &key);

        self.draw_textured_quad(
            Rect::new(
                cursor_x + glyph.offset[0],
                y + glyph.offset[1],
                glyph.tex_coords[2] - glyph.tex_coords[0],
                glyph.tex_coords[3] - glyph.tex_coords[1],
            ),
            glyph.tex_coords,
            color,
        );

        cursor_x += glyph.advance;
    }
}
```

### Phase 4: Images (~300 lines)

#### 4.1 Texture Loading

```rust
fn load_texture(&mut self, url: &str) -> Option<&wgpu::TextureView> {
    if let Some(texture) = self.texture_cache.get(url) {
        return Some(texture);
    }

    // Get from rustkit-image cache
    let image_data = rustkit_image::get_cached(url)?;

    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: image_data.width,
            height: image_data.height,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        ..Default::default()
    });

    self.queue.write_texture(...);
    self.texture_cache.insert(url.to_string(), texture);
    self.texture_cache.get(url)
}
```

#### 4.2 Image Drawing

```rust
fn draw_image(&mut self, cmd: &DisplayCommand::Image) {
    let texture = match self.load_texture(&cmd.url) {
        Some(t) => t,
        None => return,  // Image not loaded yet
    };

    let dest = cmd.object_fit.compute_rect(
        cmd.dest_rect,
        texture.width as f32,
        texture.height as f32,
        (0.5, 0.5),  // center
    );

    self.draw_textured_quad(dest.dest, [0.0, 0.0, 1.0, 1.0], Color::WHITE);
}
```

### Phase 5: Clipping & Stacking (~200 lines)

#### 5.1 Clip Stack

```rust
fn push_clip(&mut self, rect: Rect) {
    // Flush current batch
    self.flush();

    // Intersect with current clip
    let clip = if let Some(current) = self.clip_stack.last() {
        current.intersect(&rect)
    } else {
        rect
    };

    self.clip_stack.push(clip);
    self.set_scissor_rect(clip);
}

fn pop_clip(&mut self) {
    self.flush();
    self.clip_stack.pop();

    if let Some(clip) = self.clip_stack.last() {
        self.set_scissor_rect(*clip);
    } else {
        self.clear_scissor_rect();
    }
}
```

### Phase 6: Execute Display List (~200 lines)

```rust
impl Renderer {
    pub fn execute(&mut self, commands: &[DisplayCommand], target: &wgpu::TextureView) {
        for cmd in commands {
            match cmd {
                DisplayCommand::SolidColor(color, rect) => {
                    self.draw_solid_rect(*rect, *color);
                }

                DisplayCommand::Border { color, rect, widths, .. } => {
                    self.draw_border(*rect, *color, *widths);
                }

                DisplayCommand::Text { text, x, y, font_size, color, font_family, .. } => {
                    self.draw_text(text, *x, *y, *font_size, *color, font_family);
                }

                DisplayCommand::Image { url, dest_rect, object_fit, opacity, .. } => {
                    self.draw_image(url, dest_rect, object_fit, *opacity);
                }

                DisplayCommand::PushClip(rect) => self.push_clip(*rect),
                DisplayCommand::PopClip => self.pop_clip(),

                DisplayCommand::PushStackingContext { z_index, rect } => {
                    // For now, just track for debugging
                    // Full impl would use separate render targets
                }
                DisplayCommand::PopStackingContext => {}

                // SVG commands
                DisplayCommand::FillRect { rect, color } => {
                    self.draw_solid_rect(*rect, *color);
                }
                DisplayCommand::FillCircle { cx, cy, radius, color } => {
                    self.draw_circle(*cx, *cy, *radius, *color, true);
                }
                // ... other SVG commands

                _ => {
                    tracing::warn!("Unhandled display command: {:?}", cmd);
                }
            }
        }

        // Final flush
        self.flush_to(target);
    }
}
```

### Phase 7: Integration (~100 lines)

#### Update rustkit-engine

```rust
// In Engine::render()
fn render(&mut self, id: EngineViewId) -> Result<(), EngineError> {
    let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;

    // Build display list from layout
    let display_list = if let Some(ref layout) = view.layout_root {
        DisplayList::build(layout)
    } else {
        DisplayList::default()
    };

    // Get render target
    let surface = self.compositor.get_surface(view.viewhost_id)?;
    let output = surface.get_current_texture()?;
    let view = output.texture.create_view(&Default::default());

    // Execute display list
    self.renderer.execute(&display_list.commands, &view);

    output.present();
    Ok(())
}
```

---

## Dependencies

```toml
[dependencies]
wgpu = "24"
bytemuck = { version = "1", features = ["derive"] }
rustkit-layout = { path = "../rustkit-layout" }
rustkit-css = { path = "../rustkit-css" }
rustkit-image = { path = "../rustkit-image" }
dwrote = "0.11"  # For DirectWrite glyph rasterization
tracing = "0.1"
thiserror = "1.0"
```

---

## Shader Code

### color.wgsl

```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Convert from pixel coords to clip space
    out.clip_position = vec4<f32>(
        in.position.x * 2.0 / uniforms.viewport_size.x - 1.0,
        1.0 - in.position.y * 2.0 / uniforms.viewport_size.y,
        0.0,
        1.0
    );
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
```

### texture.wgsl

```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(
        in.position.x * 2.0 / uniforms.viewport_size.x - 1.0,
        1.0 - in.position.y * 2.0 / uniforms.viewport_size.y,
        0.0,
        1.0
    );
    out.tex_coords = in.tex_coords;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    return tex_color * in.color;
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_solid_rect_vertices() {
    let mut renderer = Renderer::new_for_testing();
    renderer.draw_solid_rect(Rect::new(10.0, 20.0, 100.0, 50.0), Color::RED);

    assert_eq!(renderer.color_vertices.len(), 4);
    assert_eq!(renderer.color_indices.len(), 6);
}

#[test]
fn test_clip_stack() {
    let mut renderer = Renderer::new_for_testing();

    renderer.push_clip(Rect::new(0.0, 0.0, 100.0, 100.0));
    renderer.push_clip(Rect::new(50.0, 50.0, 100.0, 100.0));

    // Should be intersection: (50, 50, 50, 50)
    let clip = renderer.current_clip().unwrap();
    assert_eq!(clip, Rect::new(50.0, 50.0, 50.0, 50.0));
}
```

### Visual Tests

Create test HTML pages that exercise each DisplayCommand type:

```
tests/visual/
├── solid_colors.html    # Various colored rectangles
├── borders.html         # Border styles and radii
├── text_basic.html      # Simple text rendering
├── text_fonts.html      # Font families and sizes
├── images.html          # Image loading and object-fit
├── clipping.html        # Overflow clipping
├── stacking.html        # Z-index ordering
└── svg_shapes.html      # SVG primitives
```

---

## Milestones

| Milestone | Deliverable | Lines | Validates |
|-----------|-------------|-------|-----------|
| M1 | Solid color rectangles render | ~300 | Pipeline works |
| M2 | Borders render | ~100 | Complex shapes |
| M3 | Text renders | ~400 | DirectWrite integration |
| M4 | Images render | ~300 | Texture system |
| M5 | Clipping works | ~200 | Scissor rects |
| M6 | Full DisplayList execution | ~200 | End-to-end |
| M7 | Engine integration | ~100 | Production ready |

**Total: ~1,600 lines**

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| DirectWrite complexity | High | Start with simple fonts, defer ligatures |
| Glyph cache overflow | Medium | LRU eviction, multiple atlas pages |
| Performance | High | Batch aggressively, profile early |
| wgpu breaking changes | Low | Pin version, test on upgrade |

---

## Definition of Done

- [x] All DisplayCommand variants handled
- [x] Text renders (fallback placeholder - TODO: DirectWrite)
- [x] Images load and display (via texture cache)
- [x] Clipping/overflow works (scissor rects)
- [ ] Visual regression tests pass (TODO)
- [x] Integrated into Engine::render()
- [ ] Simple HTML page displays correctly (requires end-to-end testing)
- [ ] Performance: 60fps for typical page (requires benchmarking)

---

## Next Steps After Renderer

Once the renderer is complete:

1. **Canvas 2D** - Can actually draw to screen
2. **SVG** - Shapes will appear
3. **WebGL** - Separate context but same pattern
4. **Animations** - Rendered values will be visible

**This is the single most important piece of missing infrastructure.**
