//! Linux Text Backend using Fontconfig + FreeType
//!
//! This module provides text shaping and font access on Linux using
//! Fontconfig for font enumeration/matching and FreeType for rendering.
//!
//! ## Features
//!
//! - Font enumeration and matching (Fontconfig)
//! - Glyph rendering (FreeType)
//! - Font fallback for missing glyphs
//! - Text metrics

#![cfg(target_os = "linux")]

use crate::{
    FontDescriptor, FontFamily, FontStyle, FontWeight, GlyphInfo, ShapedGlyph, ShapedText,
    TextBackend, TextError, TextMetrics,
};
use fontconfig::Fontconfig;
use freetype::Library;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, error, info, trace, warn};

/// Linux text backend using Fontconfig + FreeType.
pub struct LinuxTextBackend {
    /// FreeType library handle
    ft_library: Library,
    /// Fontconfig handle
    fontconfig: Fontconfig,
    /// Cache of loaded faces
    face_cache: HashMap<String, freetype::Face>,
    /// Default font size
    default_size: f32,
}

impl LinuxTextBackend {
    /// Create a new Linux text backend.
    pub fn new() -> Result<Self, TextError> {
        info!("Initializing Linux text backend (Fontconfig + FreeType)");

        let ft_library = Library::init().map_err(|e| {
            TextError::InitializationFailed(format!("Failed to initialize FreeType: {:?}", e))
        })?;

        let fontconfig = Fontconfig::new().ok_or_else(|| {
            TextError::InitializationFailed("Failed to initialize Fontconfig".into())
        })?;

        Ok(Self {
            ft_library,
            fontconfig,
            face_cache: HashMap::new(),
            default_size: 16.0,
        })
    }

    /// Find a font file matching the descriptor.
    fn find_font(&self, descriptor: &FontDescriptor) -> Result<PathBuf, TextError> {
        // Use fontconfig to find matching font
        let pattern = format!(
            "{}:weight={}:slant={}",
            descriptor.family,
            match descriptor.weight {
                w if w.0 < 400 => "light",
                w if w.0 < 600 => "regular",
                w if w.0 < 700 => "medium",
                _ => "bold",
            },
            match descriptor.style {
                FontStyle::Normal => "roman",
                FontStyle::Italic => "italic",
                FontStyle::Oblique => "oblique",
            }
        );

        self.fontconfig
            .find(&descriptor.family, None)
            .map(|font| font.path)
            .ok_or_else(|| {
                TextError::FontNotFound(format!("Font '{}' not found", descriptor.family))
            })
    }

    /// Get or load a FreeType face.
    fn get_face(&mut self, descriptor: &FontDescriptor) -> Result<&freetype::Face, TextError> {
        let key = format!(
            "{}:{}:{}",
            descriptor.family,
            descriptor.weight.0,
            match descriptor.style {
                FontStyle::Normal => "n",
                FontStyle::Italic => "i",
                FontStyle::Oblique => "o",
            }
        );

        if !self.face_cache.contains_key(&key) {
            let path = self.find_font(descriptor)?;
            let face = self
                .ft_library
                .new_face(&path, 0)
                .map_err(|e| TextError::FontNotFound(format!("Failed to load font: {:?}", e)))?;

            // Set char size
            face.set_char_size(
                (descriptor.size * 64.0) as isize, // width in 1/64 points
                (descriptor.size * 64.0) as isize, // height in 1/64 points
                72,                                 // horizontal DPI
                72,                                 // vertical DPI
            )
            .map_err(|e| TextError::ShapingFailed(format!("Failed to set char size: {:?}", e)))?;

            self.face_cache.insert(key.clone(), face);
        }

        Ok(self.face_cache.get(&key).unwrap())
    }
}

/// A CPU-rasterized glyph: 8-bit coverage bitmap plus placement metrics.
///
/// Exists so the GPU renderer can draw REAL glyphs on Linux. Until this API
/// existed, rustkit-text could measure and shape text here (Fontconfig +
/// FreeType were fully wired) but exposed no way to get pixels out — so the
/// renderer's non-Windows path drew a filled rectangle per character, which is
/// exactly what the first native screenshot showed: white blocks where words
/// belong. The capability existed one crate away from its only consumer.
#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    /// 8-bit alpha coverage, row-major, `width * height` bytes.
    pub bitmap: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Horizontal offset from pen position to bitmap left edge.
    pub bearing_x: i32,
    /// Vertical offset from BASELINE up to bitmap top edge.
    pub bearing_y: i32,
    /// Pen advance to the next glyph, in pixels.
    pub advance: f32,
    /// Typographic ascent of the face at this size, in pixels. Callers that
    /// receive a TOP-anchored y (as the display list does) place the baseline
    /// at `y + ascent`, then the bitmap top at `baseline - bearing_y`.
    pub ascent: f32,
}

impl LinuxTextBackend {
    /// Rasterize one character to an alpha coverage bitmap.
    pub fn rasterize_glyph(
        &mut self,
        ch: char,
        descriptor: &FontDescriptor,
    ) -> Result<RasterizedGlyph, TextError> {
        let face = self.get_face(descriptor)?;

        face.load_char(ch as usize, freetype::face::LoadFlag::RENDER)
            .map_err(|e| TextError::ShapingFailed(format!("load_char {ch:?}: {e:?}")))?;

        let glyph = face.glyph();
        let bmp = glyph.bitmap();
        let width = bmp.width() as u32;
        let height = bmp.rows() as u32;

        // FreeType rows may be padded; copy row-by-row honoring pitch.
        let pitch = bmp.pitch();
        let src = bmp.buffer();
        let mut bitmap = vec![0u8; (width * height) as usize];
        for row in 0..height as usize {
            let start = row * pitch.unsigned_abs() as usize;
            let end = start + width as usize;
            if end <= src.len() {
                bitmap[row * width as usize..(row + 1) * width as usize]
                    .copy_from_slice(&src[start..end]);
            }
        }

        let ascent = face
            .size_metrics()
            .map(|m| (m.ascender >> 6) as f32)
            .unwrap_or(descriptor.size * 0.8);

        Ok(RasterizedGlyph {
            bitmap,
            width,
            height,
            bearing_x: glyph.bitmap_left(),
            bearing_y: glyph.bitmap_top(),
            advance: (glyph.advance().x >> 6) as f32,
            ascent,
        })
    }
}

impl Default for LinuxTextBackend {
    fn default() -> Self {
        Self::new().expect("Failed to create Linux text backend")
    }
}

impl TextBackend for LinuxTextBackend {
    fn shape_text(&mut self, text: &str, descriptor: &FontDescriptor) -> Result<ShapedText, TextError> {
        if text.is_empty() {
            return Ok(ShapedText {
                glyphs: vec![],
                width: 0.0,
                metrics: TextMetrics::default(),
            });
        }

        let metrics = self.get_metrics(descriptor)?;
        let face = self.get_face(descriptor)?;

        let mut glyphs = Vec::new();
        let mut x_offset = 0.0f32;

        for (cluster, c) in text.chars().enumerate() {
            // get_char_index now returns Result<NonZeroU32, freetype::Error>
            let glyph_index = face
                .get_char_index(c as usize)
                .map(|nz| nz.get())
                .map_err(|e| TextError::ShapingFailed(format!("Failed to get glyph index: {:?}", e)))?;

            face.load_glyph(glyph_index, freetype::face::LoadFlag::DEFAULT)
                .map_err(|e| TextError::ShapingFailed(format!("Failed to load glyph: {:?}", e)))?;

            let glyph = face.glyph();
            let advance = glyph.advance().x as f32 / 64.0;

            glyphs.push(ShapedGlyph {
                glyph_id: glyph_index as u32,
                x_offset,
                y_offset: 0.0,
                advance,
                cluster: cluster as u32,
            });

            x_offset += advance;
        }

        trace!(
            text_len = text.len(),
            glyph_count = glyphs.len(),
            width = x_offset,
            "Shaped text with FreeType"
        );

        Ok(ShapedText {
            glyphs,
            width: x_offset,
            metrics,
        })
    }

    fn get_font_families(&self) -> Result<Vec<FontFamily>, TextError> {
        // Fontconfig can enumerate all available fonts
        // For now, return common families
        let families = vec![
            FontFamily {
                name: "DejaVu Sans".to_string(),
                styles: vec![FontStyle::Normal, FontStyle::Italic],
            },
            FontFamily {
                name: "Liberation Sans".to_string(),
                styles: vec![FontStyle::Normal, FontStyle::Italic],
            },
            FontFamily {
                name: "Noto Sans".to_string(),
                styles: vec![FontStyle::Normal, FontStyle::Italic],
            },
            FontFamily {
                name: "Ubuntu".to_string(),
                styles: vec![FontStyle::Normal, FontStyle::Italic],
            },
        ];

        Ok(families)
    }

    fn get_metrics(&mut self, descriptor: &FontDescriptor) -> Result<TextMetrics, TextError> {
        let face = self.get_face(descriptor)?;

        let size_metrics = face.size_metrics().ok_or_else(|| {
            TextError::ShapingFailed("Failed to get font metrics".into())
        })?;

        let ascent = size_metrics.ascender as f32 / 64.0;
        let descent = (size_metrics.descender as f32 / 64.0).abs();
        let height = size_metrics.height as f32 / 64.0;

        Ok(TextMetrics {
            ascent,
            descent,
            line_height: height,
            em_size: descriptor.size,
            x_height: ascent * 0.5, // Approximate
            cap_height: ascent * 0.7, // Approximate
        })
    }

    fn get_fallback_fonts(&self, text: &str) -> Vec<String> {
        let mut fallbacks = vec![];

        // Check if text contains CJK characters
        let has_cjk = text.chars().any(|c| {
            let cp = c as u32;
            (0x4E00..=0x9FFF).contains(&cp)
                || (0x3040..=0x309F).contains(&cp)
                || (0x30A0..=0x30FF).contains(&cp)
                || (0xAC00..=0xD7AF).contains(&cp)
        });

        if has_cjk {
            fallbacks.push("Noto Sans CJK SC".to_string());
            fallbacks.push("Noto Sans CJK JP".to_string());
            fallbacks.push("Noto Sans CJK KR".to_string());
        }

        // Check for emoji
        let has_emoji = text.chars().any(|c| {
            let cp = c as u32;
            (0x1F300..=0x1F9FF).contains(&cp)
        });

        if has_emoji {
            fallbacks.push("Noto Color Emoji".to_string());
        }

        // Standard fallbacks
        fallbacks.push("DejaVu Sans".to_string());
        fallbacks.push("Liberation Sans".to_string());
        fallbacks.push("Noto Sans".to_string());

        fallbacks
    }
}

#[cfg(test)]
mod tests {
    // Linux tests require X11/Wayland display and fonts
}

