//! # RustKit CSS
//!
//! CSS parsing and style computation for the RustKit browser engine.
//!
//! ## Design Goals
//!
//! 1. **Property parsing**: Parse CSS property values
//! 2. **Cascade**: Apply specificity and origin rules
//! 3. **Inheritance**: Propagate inherited properties to children
//! 4. **Computed values**: Resolve relative units and keywords

use thiserror::Error;
use tracing::debug;
use rustkit_cssparser::parse_stylesheet;

/// Errors that can occur in CSS operations.
#[derive(Error, Debug)]
pub enum CssError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),
}

/// A CSS color value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0.0,
    };
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 1.0,
    };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 1.0,
    };

    pub fn new(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Convert to [f64; 4] for rendering.
    pub fn to_f64_array(&self) -> [f64; 4] {
        [
            self.r as f64 / 255.0,
            self.g as f64 / 255.0,
            self.b as f64 / 255.0,
            self.a as f64,
        ]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// High-precision color for internal rendering calculations.
/// RGB components are stored as f32 in 0.0-1.0 range.
/// Use for gradient interpolation and internal processing.
/// Convert to Color only at final display/storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorF32 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorF32 {
    pub const TRANSPARENT: ColorF32 = ColorF32 { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const BLACK: ColorF32 = ColorF32 { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: ColorF32 = ColorF32 { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    #[inline]
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub fn from_rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Convert from 8-bit Color to high-precision ColorF32.
    #[inline]
    pub fn from_color(c: Color) -> Self {
        Self {
            r: c.r as f32 / 255.0,
            g: c.g as f32 / 255.0,
            b: c.b as f32 / 255.0,
            a: c.a,
        }
    }

    /// Convert to 8-bit Color for final display.
    /// Uses rounding for best accuracy.
    #[inline]
    pub fn to_color(&self) -> Color {
        Color {
            r: (self.r * 255.0).round().clamp(0.0, 255.0) as u8,
            g: (self.g * 255.0).round().clamp(0.0, 255.0) as u8,
            b: (self.b * 255.0).round().clamp(0.0, 255.0) as u8,
            a: self.a,
        }
    }

    /// Convert to 8-bit Color with ordered dithering to reduce banding.
    /// `pixel_x` and `pixel_y` are the screen coordinates for dither pattern.
    #[inline]
    pub fn to_color_dithered(&self, pixel_x: u32, pixel_y: u32) -> Color {
        // 4x4 Bayer ordered dithering matrix (normalized to 0.0-1.0 range)
        const BAYER_4X4: [[f32; 4]; 4] = [
            [0.0/16.0, 8.0/16.0, 2.0/16.0, 10.0/16.0],
            [12.0/16.0, 4.0/16.0, 14.0/16.0, 6.0/16.0],
            [3.0/16.0, 11.0/16.0, 1.0/16.0, 9.0/16.0],
            [15.0/16.0, 7.0/16.0, 13.0/16.0, 5.0/16.0],
        ];

        let dither = BAYER_4X4[(pixel_y & 3) as usize][(pixel_x & 3) as usize];
        let dither_offset = (dither - 0.5) / 255.0;

        Color {
            r: ((self.r + dither_offset) * 255.0).round().clamp(0.0, 255.0) as u8,
            g: ((self.g + dither_offset) * 255.0).round().clamp(0.0, 255.0) as u8,
            b: ((self.b + dither_offset) * 255.0).round().clamp(0.0, 255.0) as u8,
            a: self.a,
        }
    }

    /// Linear interpolation between two colors using premultiplied alpha.
    /// Chrome/Skia uses premultiplied alpha interpolation for gradients, which
    /// prevents color bleeding from transparent color stops.
    #[inline]
    pub fn lerp(&self, other: &ColorF32, t: f32) -> ColorF32 {
        // Convert to premultiplied alpha
        let pre1_r = self.r * self.a;
        let pre1_g = self.g * self.a;
        let pre1_b = self.b * self.a;

        let pre2_r = other.r * other.a;
        let pre2_g = other.g * other.a;
        let pre2_b = other.b * other.a;

        // Interpolate in premultiplied space
        let pre_r = pre1_r + (pre2_r - pre1_r) * t;
        let pre_g = pre1_g + (pre2_g - pre1_g) * t;
        let pre_b = pre1_b + (pre2_b - pre1_b) * t;
        let a = self.a + (other.a - self.a) * t;

        // Convert back from premultiplied (avoid division by zero)
        if a > 0.0001 {
            ColorF32 {
                r: pre_r / a,
                g: pre_g / a,
                b: pre_b / a,
                a,
            }
        } else {
            // Fully transparent - color doesn't matter
            ColorF32 { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }
        }
    }

    /// Linear interpolation using straight (unpremultiplied) alpha.
    /// Use this when premultiplied interpolation is not desired.
    #[inline]
    pub fn lerp_straight(&self, other: &ColorF32, t: f32) -> ColorF32 {
        ColorF32 {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Gamma-correct interpolation for CSS gradients.
    /// Converts sRGB to linear space, interpolates in premultiplied linear,
    /// then converts back to sRGB. This matches Chrome's gradient rendering.
    #[inline]
    pub fn lerp_gamma_correct(&self, other: &ColorF32, t: f32) -> ColorF32 {
        // Convert sRGB to linear
        let l1_r = Self::srgb_to_linear(self.r);
        let l1_g = Self::srgb_to_linear(self.g);
        let l1_b = Self::srgb_to_linear(self.b);

        let l2_r = Self::srgb_to_linear(other.r);
        let l2_g = Self::srgb_to_linear(other.g);
        let l2_b = Self::srgb_to_linear(other.b);

        // Premultiply by alpha in linear space
        let pre1_r = l1_r * self.a;
        let pre1_g = l1_g * self.a;
        let pre1_b = l1_b * self.a;

        let pre2_r = l2_r * other.a;
        let pre2_g = l2_g * other.a;
        let pre2_b = l2_b * other.a;

        // Interpolate in linear premultiplied space
        let pre_r = pre1_r + (pre2_r - pre1_r) * t;
        let pre_g = pre1_g + (pre2_g - pre1_g) * t;
        let pre_b = pre1_b + (pre2_b - pre1_b) * t;
        let a = self.a + (other.a - self.a) * t;

        // Convert back from premultiplied and to sRGB
        if a > 0.0001 {
            ColorF32 {
                r: Self::linear_to_srgb(pre_r / a),
                g: Self::linear_to_srgb(pre_g / a),
                b: Self::linear_to_srgb(pre_b / a),
                a,
            }
        } else {
            ColorF32 { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }
        }
    }

    /// Convert sRGB to linear space.
    #[inline]
    fn srgb_to_linear(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Convert linear to sRGB space.
    #[inline]
    fn linear_to_srgb(c: f32) -> f32 {
        if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    }

    /// Convert to array for GPU vertex buffers.
    #[inline]
    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

/// A CSS length value.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    /// Pixels.
    Px(f32),
    /// Em (relative to font size).
    Em(f32),
    /// Rem (relative to root font size).
    Rem(f32),
    /// Percentage.
    Percent(f32),
    /// Auto.
    Auto,
    /// Zero.
    #[default]
    Zero,
}

impl Length {
    /// Compute the absolute pixel value.
    pub fn to_px(&self, font_size: f32, root_font_size: f32, container_size: f32) -> f32 {
        match self {
            Length::Px(px) => *px,
            Length::Em(em) => em * font_size,
            Length::Rem(rem) => rem * root_font_size,
            Length::Percent(pct) => pct / 100.0 * container_size,
            Length::Auto => 0.0, // Context-dependent
            Length::Zero => 0.0,
        }
    }
}

/// Display property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Block,
    Inline,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    None,
}

impl Display {
    /// Check if this is a flex container.
    pub fn is_flex(self) -> bool {
        matches!(self, Display::Flex | Display::InlineFlex)
    }

    /// Check if this is a grid container.
    pub fn is_grid(self) -> bool {
        matches!(self, Display::Grid | Display::InlineGrid)
    }

    /// Check if this is an inline-level display (inline, inline-block, inline-flex, inline-grid).
    pub fn is_inline_level(self) -> bool {
        matches!(self, Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid)
    }

    /// Check if this is inline-block.
    pub fn is_inline_block(self) -> bool {
        matches!(self, Display::InlineBlock)
    }

    /// Check if this is an atomic inline-level box (inline-block, inline-flex,
    /// inline-grid): participates in inline flow as a single opaque box while
    /// laying out its own contents with its inner display type (CSS Display 3
    /// §2.4).
    pub fn is_atomic_inline(self) -> bool {
        matches!(self, Display::InlineBlock | Display::InlineFlex | Display::InlineGrid)
    }
}

// ==================== Flexbox Types ====================

/// Flex direction property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

impl FlexDirection {
    /// Check if this direction is reversed.
    pub fn is_reverse(self) -> bool {
        matches!(self, FlexDirection::RowReverse | FlexDirection::ColumnReverse)
    }

    /// Check if this is a row direction.
    pub fn is_row(self) -> bool {
        matches!(self, FlexDirection::Row | FlexDirection::RowReverse)
    }

    /// Check if this is a column direction.
    pub fn is_column(self) -> bool {
        matches!(self, FlexDirection::Column | FlexDirection::ColumnReverse)
    }
}

/// Flex wrap property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Justify content property (main axis alignment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Align items property (cross axis alignment for all items).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// Align content property (multi-line cross axis alignment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignContent {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Align self property (cross axis alignment for individual item).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignSelf {
    #[default]
    Auto,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

/// Flex basis property.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FlexBasis {
    /// Use the item's main size property (width or height).
    #[default]
    Auto,
    /// Size based on content.
    Content,
    /// Explicit length.
    Length(f32),
    /// Percentage of container.
    Percent(f32),
}

// ==================== Grid Types ====================

/// A grid track size.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackSize {
    /// Fixed length in pixels.
    Px(f32),
    /// Percentage of container.
    Percent(f32),
    /// Fractional unit (flexible).
    Fr(f32),
    /// Size based on content minimum.
    MinContent,
    /// Size based on content maximum.
    MaxContent,
    /// Auto sizing.
    Auto,
    /// Minimum/maximum constraint.
    MinMax(Box<TrackSize>, Box<TrackSize>),
    /// Fit content with maximum.
    FitContent(f32),
}

impl Default for TrackSize {
    fn default() -> Self {
        TrackSize::Auto
    }
}

impl TrackSize {
    /// Create a fixed pixel size.
    pub fn px(value: f32) -> Self {
        TrackSize::Px(value)
    }

    /// Create a fractional size.
    pub fn fr(value: f32) -> Self {
        TrackSize::Fr(value)
    }

    /// Create a minmax constraint.
    pub fn minmax(min: TrackSize, max: TrackSize) -> Self {
        TrackSize::MinMax(Box::new(min), Box::new(max))
    }

    /// Check if this is a flexible track (contains fr units).
    pub fn is_flexible(&self) -> bool {
        match self {
            TrackSize::Fr(_) => true,
            TrackSize::MinMax(_, max) => max.is_flexible(),
            _ => false,
        }
    }

    /// Get the minimum size contribution.
    pub fn min_size(&self) -> f32 {
        match self {
            TrackSize::Px(v) => *v,
            TrackSize::MinMax(min, _) => min.min_size(),
            TrackSize::FitContent(max) => 0.0_f32.min(*max),
            _ => 0.0,
        }
    }
}

/// A grid track definition (for grid-template-columns/rows).
#[derive(Debug, Clone, PartialEq)]
pub struct TrackDefinition {
    /// Track sizing.
    pub size: TrackSize,
    /// Optional line name(s) before this track.
    pub line_names: Vec<String>,
}

impl TrackDefinition {
    /// Create a simple track without line names.
    pub fn simple(size: TrackSize) -> Self {
        Self {
            size,
            line_names: Vec::new(),
        }
    }

    /// Create a track with line name.
    pub fn named(size: TrackSize, name: &str) -> Self {
        Self {
            size,
            line_names: vec![name.to_string()],
        }
    }
}

/// Repeat function for grid tracks.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackRepeat {
    /// Repeat a fixed number of times.
    Count(u32, Vec<TrackDefinition>),
    /// Auto-fill: as many as fit.
    AutoFill(Vec<TrackDefinition>),
    /// Auto-fit: as many as fit, collapsing empty tracks.
    AutoFit(Vec<TrackDefinition>),
}

/// Grid template definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridTemplate {
    /// Explicit track definitions.
    pub tracks: Vec<TrackDefinition>,
    /// Repeat patterns.
    pub repeats: Vec<(usize, TrackRepeat)>, // (insert_position, repeat)
    /// Final line names.
    pub final_line_names: Vec<String>,
}

impl GridTemplate {
    /// Create an empty template (no explicit tracks).
    pub fn none() -> Self {
        Self::default()
    }

    /// Create from a list of track sizes.
    pub fn from_sizes(sizes: Vec<TrackSize>) -> Self {
        Self {
            tracks: sizes.into_iter().map(TrackDefinition::simple).collect(),
            repeats: Vec::new(),
            final_line_names: Vec::new(),
        }
    }

    /// Get the number of explicit tracks.
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }
}

/// Named grid area.
#[derive(Debug, Clone, PartialEq)]
pub struct GridArea {
    pub name: String,
    pub row_start: i32,
    pub row_end: i32,
    pub column_start: i32,
    pub column_end: i32,
}

/// Grid template areas.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridTemplateAreas {
    /// Row strings (e.g., ["header header", "nav main", "footer footer"]).
    pub rows: Vec<Vec<Option<String>>>,
    /// Named areas derived from rows.
    pub areas: Vec<GridArea>,
}

impl GridTemplateAreas {
    /// Parse grid-template-areas value.
    pub fn parse(value: &str) -> Option<Self> {
        let mut rows = Vec::new();
        
        for line in value.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Remove quotes if present
            let line = line.trim_matches('"').trim_matches('\'');
            
            let cells: Vec<Option<String>> = line
                .split_whitespace()
                .map(|s| {
                    if s == "." {
                        None
                    } else {
                        Some(s.to_string())
                    }
                })
                .collect();
            
            rows.push(cells);
        }

        if rows.is_empty() {
            return None;
        }

        // Extract named areas
        let mut areas = Vec::new();
        let mut area_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        for (row_idx, row) in rows.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                if let Some(name) = cell {
                    if !area_names.contains(name) {
                        // Find extent of this area
                        let (row_end, col_end) = Self::find_area_extent(&rows, row_idx, col_idx, name);
                        areas.push(GridArea {
                            name: name.clone(),
                            row_start: row_idx as i32 + 1,
                            row_end: row_end as i32 + 1,
                            column_start: col_idx as i32 + 1,
                            column_end: col_end as i32 + 1,
                        });
                        area_names.insert(name.clone());
                    }
                }
            }
        }

        Some(Self { rows, areas })
    }

    fn find_area_extent(rows: &[Vec<Option<String>>], start_row: usize, start_col: usize, name: &str) -> (usize, usize) {
        let mut row_end = start_row;
        let mut col_end = start_col;

        // Find column extent
        for col in start_col..rows[start_row].len() {
            if rows[start_row].get(col) == Some(&Some(name.to_string())) {
                col_end = col + 1;
            } else {
                break;
            }
        }

        // Find row extent
        for row in start_row..rows.len() {
            if rows[row].get(start_col) == Some(&Some(name.to_string())) {
                row_end = row + 1;
            } else {
                break;
            }
        }

        (row_end, col_end)
    }

    /// Get area by name.
    pub fn get_area(&self, name: &str) -> Option<&GridArea> {
        self.areas.iter().find(|a| a.name == name)
    }
}

/// Grid auto flow direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

impl GridAutoFlow {
    /// Check if this is a row-based flow.
    pub fn is_row(self) -> bool {
        matches!(self, GridAutoFlow::Row | GridAutoFlow::RowDense)
    }

    /// Check if this uses dense packing.
    pub fn is_dense(self) -> bool {
        matches!(self, GridAutoFlow::RowDense | GridAutoFlow::ColumnDense)
    }
}

/// Grid line reference (for grid-column-start, etc.).
#[derive(Debug, Clone, PartialEq)]
pub enum GridLine {
    /// Auto placement.
    Auto,
    /// Specific line number (1-based, can be negative).
    Number(i32),
    /// Named line.
    Name(String),
    /// Span a number of tracks.
    Span(u32),
    /// Span to a named line.
    SpanName(String),
}

impl Default for GridLine {
    fn default() -> Self {
        GridLine::Auto
    }
}

/// Grid placement for an item.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridPlacement {
    /// Column start line.
    pub column_start: GridLine,
    /// Column end line.
    pub column_end: GridLine,
    /// Row start line.
    pub row_start: GridLine,
    /// Row end line.
    pub row_end: GridLine,
}

impl GridPlacement {
    /// Create placement from a named area.
    pub fn from_area(name: &str) -> Self {
        Self {
            column_start: GridLine::Name(format!("{}-start", name)),
            column_end: GridLine::Name(format!("{}-end", name)),
            row_start: GridLine::Name(format!("{}-start", name)),
            row_end: GridLine::Name(format!("{}-end", name)),
        }
    }

    /// Create placement from explicit lines.
    pub fn from_lines(col_start: i32, col_end: i32, row_start: i32, row_end: i32) -> Self {
        Self {
            column_start: GridLine::Number(col_start),
            column_end: GridLine::Number(col_end),
            row_start: GridLine::Number(row_start),
            row_end: GridLine::Number(row_end),
        }
    }
}

/// Justify items (horizontal alignment in grid cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyItems {
    #[default]
    Stretch,
    Start,
    End,
    Center,
}

/// Justify self (horizontal alignment for individual item).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifySelf {
    #[default]
    Auto,
    Stretch,
    Start,
    End,
    Center,
}

/// Position property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// Font weight values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const NORMAL: FontWeight = FontWeight(400);
    pub const BOLD: FontWeight = FontWeight(700);
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Font style values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
    Center,
    Justify,
}

/// Overflow behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Auto,
    Clip,
}

impl Overflow {
    /// Check if this overflow creates a scroll container.
    pub fn is_scrollable(self) -> bool {
        matches!(self, Overflow::Scroll | Overflow::Auto)
    }

    /// Check if content is clipped.
    pub fn clips_content(self) -> bool {
        !matches!(self, Overflow::Visible)
    }
}

/// Scroll behavior for smooth scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBehavior {
    #[default]
    Auto,
    Smooth,
}

/// Overscroll behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverscrollBehavior {
    #[default]
    Auto,
    Contain,
    None,
}

/// Scrollbar width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarWidth {
    #[default]
    Auto,
    Thin,
    None,
}

/// Scrollbar gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarGutter {
    #[default]
    Auto,
    Stable,
    BothEdges,
}

/// Text decoration line values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextDecorationLine {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
}

impl TextDecorationLine {
    pub const NONE: TextDecorationLine = TextDecorationLine {
        underline: false,
        overline: false,
        line_through: false,
    };

    pub const UNDERLINE: TextDecorationLine = TextDecorationLine {
        underline: true,
        overline: false,
        line_through: false,
    };

    pub const OVERLINE: TextDecorationLine = TextDecorationLine {
        underline: false,
        overline: true,
        line_through: false,
    };

    pub const LINE_THROUGH: TextDecorationLine = TextDecorationLine {
        underline: false,
        overline: false,
        line_through: true,
    };
}

/// Text decoration style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDecorationStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// Font stretch values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    #[default]
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl FontStretch {
    /// Convert to DirectWrite font stretch value (1-9).
    pub fn to_dwrite_value(&self) -> u32 {
        match self {
            FontStretch::UltraCondensed => 1,
            FontStretch::ExtraCondensed => 2,
            FontStretch::Condensed => 3,
            FontStretch::SemiCondensed => 4,
            FontStretch::Normal => 5,
            FontStretch::SemiExpanded => 6,
            FontStretch::Expanded => 7,
            FontStretch::ExtraExpanded => 8,
            FontStretch::UltraExpanded => 9,
        }
    }
}

/// White space handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

/// Word break behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WordBreak {
    #[default]
    Normal,
    BreakAll,
    KeepAll,
    BreakWord,
}

/// Vertical alignment.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum VerticalAlign {
    #[default]
    Baseline,
    Sub,
    Super,
    Top,
    TextTop,
    Middle,
    Bottom,
    TextBottom,
    Length(f32),
}

/// Writing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

/// Text transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextTransform {
    #[default]
    None,
    Capitalize,
    Uppercase,
    Lowercase,
}

/// Direction for bidi text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// Computed style for an element.
#[derive(Debug, Clone, Default)]
pub struct ComputedStyle {
    // Box model
    pub display: Display,
    pub position: Position,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,

    // Margin
    pub margin_top: Length,
    pub margin_right: Length,
    pub margin_bottom: Length,
    pub margin_left: Length,

    // Padding
    pub padding_top: Length,
    pub padding_right: Length,
    pub padding_bottom: Length,
    pub padding_left: Length,

    // Border
    pub border_top_width: Length,
    pub border_right_width: Length,
    pub border_bottom_width: Length,
    pub border_left_width: Length,
    pub border_top_color: Color,
    pub border_right_color: Color,
    pub border_bottom_color: Color,
    pub border_left_color: Color,

    // Colors
    pub color: Color,
    pub background_color: Color,

    // Typography - Basic
    pub font_size: Length,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_family: String,
    pub line_height: f32,
    pub text_align: TextAlign,

    // Typography - Advanced
    pub font_stretch: FontStretch,
    pub letter_spacing: Length,
    pub word_spacing: Length,
    pub text_indent: Length,
    pub text_decoration_line: TextDecorationLine,
    pub text_decoration_color: Option<Color>,
    pub text_decoration_style: TextDecorationStyle,
    pub text_decoration_thickness: Length,
    pub text_transform: TextTransform,
    pub white_space: WhiteSpace,
    pub word_break: WordBreak,
    pub vertical_align: VerticalAlign,
    pub writing_mode: WritingMode,
    pub direction: Direction,

    // Visual
    pub opacity: f32,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,

    // Flexbox Container
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub row_gap: Length,
    pub column_gap: Length,

    // Flexbox Item
    pub order: i32,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: FlexBasis,
    pub align_self: AlignSelf,

    // Scrolling
    pub scroll_behavior: ScrollBehavior,
    pub overscroll_behavior_x: OverscrollBehavior,
    pub overscroll_behavior_y: OverscrollBehavior,
    pub scrollbar_width: ScrollbarWidth,
    pub scrollbar_gutter: ScrollbarGutter,
    pub scrollbar_color: Option<(Color, Color)>, // (thumb, track)

    // Grid Container
    pub grid_template_columns: GridTemplate,
    pub grid_template_rows: GridTemplate,
    pub grid_template_areas: Option<GridTemplateAreas>,
    pub grid_auto_columns: TrackSize,
    pub grid_auto_rows: TrackSize,
    pub grid_auto_flow: GridAutoFlow,

    // Grid Item
    pub grid_column_start: GridLine,
    pub grid_column_end: GridLine,
    pub grid_row_start: GridLine,
    pub grid_row_end: GridLine,

    // Grid Alignment (also used by Flexbox)
    pub justify_items: JustifyItems,
    pub justify_self: JustifySelf,
}

impl ComputedStyle {
    /// Create default style.
    pub fn new() -> Self {
        Self {
            font_size: Length::Px(16.0),
            line_height: 1.2,
            opacity: 1.0,
            color: Color::BLACK,
            background_color: Color::TRANSPARENT,
            font_family: "sans-serif".to_string(),
            text_decoration_line: TextDecorationLine::NONE,
            text_decoration_color: None,
            text_decoration_thickness: Length::Auto,
            // Flexbox item defaults
            flex_shrink: 1.0, // Default is 1, not 0
            // Width/height default to auto (fill available space), matching
            // hiwave-macos ComputedStyle::new. Deriving these from
            // Length::default() (Zero) makes every unstyled element 0x0 and
            // suppresses parent/last-child margin collapsing (height != auto).
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Zero,
            min_height: Length::Zero,
            max_width: Length::Auto, // No max constraint
            max_height: Length::Auto,
            ..Default::default()
        }
    }

    /// Create style with inheritance from parent.
    pub fn inherit_from(parent: &ComputedStyle) -> Self {
        Self {
            // Inherited properties
            color: parent.color,
            font_size: parent.font_size,
            font_weight: parent.font_weight,
            font_style: parent.font_style,
            font_stretch: parent.font_stretch,
            font_family: parent.font_family.clone(),
            line_height: parent.line_height,
            text_align: parent.text_align,
            letter_spacing: parent.letter_spacing,
            word_spacing: parent.word_spacing,
            text_indent: parent.text_indent,
            text_transform: parent.text_transform,
            white_space: parent.white_space,
            word_break: parent.word_break,
            direction: parent.direction,
            writing_mode: parent.writing_mode,

            // Text decoration is NOT inherited (each element sets its own)
            text_decoration_line: TextDecorationLine::NONE,
            text_decoration_color: None,
            text_decoration_style: TextDecorationStyle::Solid,
            text_decoration_thickness: Length::Auto,

            // Non-inherited get defaults
            ..Default::default()
        }
    }
}

/// CSS property value (unparsed or parsed).
#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// Inherit from parent.
    Inherit,
    /// Initial value.
    Initial,
    /// Specific value.
    Specified(String),
}

/// A CSS declaration (property: value).
#[derive(Debug, Clone)]
pub struct Declaration {
    pub property: String,
    pub value: PropertyValue,
    pub important: bool,
}

/// A CSS rule (selector + declarations).
#[derive(Debug, Clone)]
pub struct Rule {
    pub selector: String,
    pub declarations: Vec<Declaration>,
}

/// A complete stylesheet.
#[derive(Debug, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

impl Stylesheet {
    /// Create an empty stylesheet.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Parse a CSS string into a stylesheet.
    pub fn parse(css: &str) -> Result<Self, CssError> {
        debug!(len = css.len(), "Parsing CSS");
        let ast = parse_stylesheet(css).map_err(|e| CssError::ParseError(e.to_string()))?;

        let rules = ast
            .rules
            .into_iter()
            .map(|r| Rule {
                selector: r.selector,
                declarations: r
                    .declarations
                    .into_iter()
                    .map(|d| Declaration {
                        property: d.property,
                        value: PropertyValue::Specified(d.value),
                        important: d.important,
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();

        debug!(rule_count = rules.len(), "CSS parsed");
        Ok(Stylesheet { rules })
    }

    /// Get the number of rules in this stylesheet.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

/// Parse a color value.
pub fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim();

    // Named colors
    match value.to_lowercase().as_str() {
        "transparent" => return Some(Color::TRANSPARENT),
        "black" => return Some(Color::BLACK),
        "white" => return Some(Color::WHITE),
        "red" => return Some(Color::from_rgb(255, 0, 0)),
        "green" => return Some(Color::from_rgb(0, 128, 0)),
        "blue" => return Some(Color::from_rgb(0, 0, 255)),
        "yellow" => return Some(Color::from_rgb(255, 255, 0)),
        "gray" | "grey" => return Some(Color::from_rgb(128, 128, 128)),
        _ => {}
    }

    // Hex colors
    if let Some(hex) = value.strip_prefix('#') {
        let (r, g, b, a) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                (r, g, b, 1.0)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r, g, b, 1.0)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0;
                (r, g, b, a)
            }
            _ => return None,
        };
        return Some(Color::new(r, g, b, a));
    }

    // rgb() / rgba()
    if value.starts_with("rgb") {
        // Simplified parsing
        let inner = value
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            let a = if parts.len() >= 4 {
                parts[3].trim().parse::<f32>().ok()?
            } else {
                1.0
            };
            return Some(Color::new(r, g, b, a));
        }
    }

    None
}

/// Parse a length value.
pub fn parse_length(value: &str) -> Option<Length> {
    let value = value.trim();

    if value == "auto" {
        return Some(Length::Auto);
    }
    if value == "0" {
        return Some(Length::Zero);
    }

    if value.ends_with("px") {
        let num = value.trim_end_matches("px").parse::<f32>().ok()?;
        return Some(Length::Px(num));
    }
    // rem MUST be checked before em: "2rem".ends_with("em") is true, so an
    // em-first order trims "em" -> "2r", fails to parse, and the ? drops the
    // whole declaration silently. (Matches macOS rustkit-css.)
    if value.ends_with("rem") {
        let num = value.trim_end_matches("rem").parse::<f32>().ok()?;
        return Some(Length::Rem(num));
    }
    if value.ends_with("em") {
        let num = value.trim_end_matches("em").parse::<f32>().ok()?;
        return Some(Length::Em(num));
    }
    if value.ends_with('%') {
        let num = value.trim_end_matches('%').parse::<f32>().ok()?;
        return Some(Length::Percent(num));
    }

    // Try plain number (treated as px)
    if let Ok(num) = value.parse::<f32>() {
        return Some(Length::Px(num));
    }

    None
}

/// Parse display value.
pub fn parse_display(value: &str) -> Option<Display> {
    match value.trim().to_lowercase().as_str() {
        "block" => Some(Display::Block),
        "inline" => Some(Display::Inline),
        "inline-block" => Some(Display::InlineBlock),
        "flex" => Some(Display::Flex),
        "none" => Some(Display::None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ColorF32 (inert type port from hiwave-macos). Pure-method execute tests —
    // parser and ComputedStyle are untouched by this port.
    #[test]
    fn test_colorf32_color_roundtrip() {
        let c = Color::new(128, 64, 200, 1.0);
        let f = ColorF32::from_color(c);
        // 128/255 ≈ 0.502
        assert!((f.r - 0.502).abs() < 0.01);
        let back = f.to_color();
        assert_eq!(back.r, 128);
        assert_eq!(back.g, 64);
        assert_eq!(back.b, 200);
    }

    #[test]
    fn test_colorf32_consts_and_array() {
        assert_eq!(ColorF32::WHITE.to_array(), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(ColorF32::TRANSPARENT.a, 0.0);
        assert_eq!(ColorF32::BLACK.to_array(), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_colorf32_lerp_midpoint() {
        let a = ColorF32::new(0.0, 0.0, 0.0, 1.0);
        let b = ColorF32::new(1.0, 1.0, 1.0, 1.0);
        // Opaque endpoints: premultiplied lerp midpoint is 0.5 grey.
        let mid = a.lerp(&b, 0.5);
        assert!((mid.r - 0.5).abs() < 1e-6);
        assert!((mid.a - 1.0).abs() < 1e-6);
        // Straight lerp agrees for opaque colors.
        let mid_s = a.lerp_straight(&b, 0.5);
        assert!((mid_s.g - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_colorf32_gamma_correct_endpoints() {
        let a = ColorF32::new(0.2, 0.4, 0.6, 1.0);
        let b = ColorF32::new(0.8, 0.6, 0.4, 1.0);
        // t=0 and t=1 must return the endpoints (round-trip through linear space).
        let at0 = a.lerp_gamma_correct(&b, 0.0);
        assert!((at0.r - 0.2).abs() < 1e-4 && (at0.b - 0.6).abs() < 1e-4);
        let at1 = a.lerp_gamma_correct(&b, 1.0);
        assert!((at1.r - 0.8).abs() < 1e-4 && (at1.b - 0.4).abs() < 1e-4);
    }

    #[test]
    fn test_parse_color_hex() {
        assert_eq!(parse_color("#fff"), Some(Color::from_rgb(255, 255, 255)));
        assert_eq!(parse_color("#000000"), Some(Color::BLACK));
        assert_eq!(parse_color("#ff0000"), Some(Color::from_rgb(255, 0, 0)));
    }

    #[test]
    fn test_parse_color_named() {
        assert_eq!(parse_color("red"), Some(Color::from_rgb(255, 0, 0)));
        assert_eq!(parse_color("black"), Some(Color::BLACK));
        assert_eq!(parse_color("transparent"), Some(Color::TRANSPARENT));
    }

    #[test]
    fn test_parse_length() {
        assert_eq!(parse_length("10px"), Some(Length::Px(10.0)));
        assert_eq!(parse_length("1.5em"), Some(Length::Em(1.5)));
        assert_eq!(parse_length("50%"), Some(Length::Percent(50.0)));
        assert_eq!(parse_length("auto"), Some(Length::Auto));
    }

    #[test]
    fn test_rem_parsed_before_em() {
        // rem MUST be checked before em: "2rem".ends_with("em") is true, so a
        // naive em-first order trims "em" -> "2r", fails to parse, and the ?
        // drops the whole declaration -> every rem length silently vanished.
        assert_eq!(parse_length("2rem"), Some(Length::Rem(2.0)));
        assert_eq!(parse_length("1.25rem"), Some(Length::Rem(1.25)));
        // Guard against the naive reorder swallowing em.
        assert_eq!(parse_length("2em"), Some(Length::Em(2.0)));
    }

    #[test]
    fn test_parse_stylesheet() {
        let css = r#"
            body {
                color: black;
            }
            .container {
                width: 100%;
            }
        "#;

        let stylesheet = Stylesheet::parse(css).unwrap();
        assert!(stylesheet.rules.len() >= 2);
    }

    #[test]
    fn test_computed_style_inherit() {
        let parent = ComputedStyle {
            color: Color::from_rgb(255, 0, 0),
            font_size: Length::Px(20.0),
            ..Default::default()
        };

        let child = ComputedStyle::inherit_from(&parent);
        assert_eq!(child.color, parent.color);
        assert_eq!(child.font_size, parent.font_size);
        // Non-inherited properties should be default
        assert_eq!(child.display, Display::Block);
    }
}
