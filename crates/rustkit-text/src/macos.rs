//! macOS Text Backend using Core Text
//!
//! This module provides text shaping and font access on macOS using Apple's
//! Core Text framework.
//!
//! ## Features
//!
//! - Font enumeration and matching
//! - Unicode text shaping (via Core Text)
//! - Font fallback for missing glyphs
//! - Text metrics (ascent, descent, line height)

#![cfg(target_os = "macos")]

use crate::{
    FontDescriptor, FontFamily, FontStyle, FontWeight, GlyphInfo, ShapedGlyph, ShapedText,
    TextBackend, TextError, TextMetrics,
};
use core_foundation::attributed_string::CFMutableAttributedString;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_graphics::font::CGFont;
use core_text::font::CTFont;
use core_text::font_descriptor::{
    kCTFontFamilyNameAttribute, kCTFontTraitsAttribute, CTFontDescriptor,
};
use core_text::line::CTLine;
use std::collections::HashMap;
use tracing::{debug, error, info, trace, warn};

/// macOS text backend using Core Text.
pub struct CoreTextBackend {
    /// Cache of loaded fonts
    font_cache: HashMap<FontCacheKey, CTFont>,
    /// Default font size
    default_size: f32,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct FontCacheKey {
    family: String,
    weight: u16,
    style: FontStyle,
    size_tenths: u32, // Size in 1/10 points for hashing
}

impl CoreTextBackend {
    /// Create a new Core Text backend.
    pub fn new() -> Result<Self, TextError> {
        info!("Initializing Core Text backend");

        Ok(Self {
            font_cache: HashMap::new(),
            default_size: 16.0,
        })
    }

    /// Get or load a font from the cache.
    fn get_font(&mut self, descriptor: &FontDescriptor) -> Result<&CTFont, TextError> {
        let key = FontCacheKey {
            family: descriptor.family.clone(),
            weight: descriptor.weight.0,
            style: descriptor.style,
            size_tenths: (descriptor.size * 10.0) as u32,
        };

        if !self.font_cache.contains_key(&key) {
            let font = self.load_font(descriptor)?;
            self.font_cache.insert(key.clone(), font);
        }

        Ok(self.font_cache.get(&key).unwrap())
    }

    /// Load a font matching the descriptor.
    fn load_font(&self, descriptor: &FontDescriptor) -> Result<CTFont, TextError> {
        let family_name = CFString::new(&descriptor.family);
        let size = descriptor.size as f64;

        // Create font with family name and size
        let font = CTFont::new_from_name(&family_name, size).map_err(|_| {
            TextError::FontNotFound(format!("Font family '{}' not found", descriptor.family))
        })?;

        debug!(
            family = %descriptor.family,
            size = descriptor.size,
            "Loaded Core Text font"
        );

        Ok(font)
    }

    /// Get font metrics for a font.
    fn get_metrics(&self, font: &CTFont) -> TextMetrics {
        let ascent = font.ascent() as f32;
        let descent = font.descent() as f32;
        let leading = font.leading() as f32;
        let units_per_em = font.units_per_em() as f32;
        let size = font.pt_size() as f32;

        TextMetrics {
            ascent,
            descent: descent.abs(),
            line_height: ascent + descent.abs() + leading,
            em_size: size,
            x_height: (units_per_em * 0.5 * size / units_per_em), // Approximate
            cap_height: (units_per_em * 0.7 * size / units_per_em), // Approximate
        }
    }
}

impl Default for CoreTextBackend {
    fn default() -> Self {
        Self::new().expect("Failed to create Core Text backend")
    }
}

impl TextBackend for CoreTextBackend {
    fn shape_text(&mut self, text: &str, descriptor: &FontDescriptor) -> Result<ShapedText, TextError> {
        if text.is_empty() {
            return Ok(ShapedText {
                glyphs: vec![],
                width: 0.0,
                metrics: TextMetrics::default(),
            });
        }

        let font = self.get_font(descriptor)?;
        let metrics = self.get_metrics(font);

        // Create attributed string with font
        let cf_string = CFString::new(text);
        let attributed_string =
            CFMutableAttributedString::new_with_string(cf_string.as_concrete_TypeRef());

        // Create line for measurement
        let line = CTLine::new_with_attributed_string(attributed_string.as_concrete_TypeRef());

        let width = line.get_typographic_bounds().width as f32;

        // Get glyph runs
        let glyph_runs = line.glyph_runs();
        let mut glyphs = Vec::new();
        let mut x_offset = 0.0f32;

        for run in glyph_runs.iter() {
            let glyph_count = run.glyph_count() as usize;
            let run_glyphs = run.glyphs();
            let positions = run.positions();

            for i in 0..glyph_count {
                let glyph_id = run_glyphs[i];
                let position = positions[i];

                glyphs.push(ShapedGlyph {
                    glyph_id: glyph_id as u32,
                    x_offset: x_offset + position.x as f32,
                    y_offset: position.y as f32,
                    advance: 0.0, // Will be computed
                    cluster: i as u32, // Simplified cluster mapping
                });
            }
        }

        // Compute advances
        for i in 0..glyphs.len() {
            if i + 1 < glyphs.len() {
                glyphs[i].advance = glyphs[i + 1].x_offset - glyphs[i].x_offset;
            } else {
                glyphs[i].advance = width - glyphs[i].x_offset;
            }
        }

        trace!(
            text_len = text.len(),
            glyph_count = glyphs.len(),
            width,
            "Shaped text with Core Text"
        );

        Ok(ShapedText {
            glyphs,
            width,
            metrics,
        })
    }

    fn get_font_families(&self) -> Result<Vec<FontFamily>, TextError> {
        // Get all available font family names
        let collection = core_text::font_collection::create_for_all_families();
        let descriptors = collection.get_descriptors();

        let mut families = Vec::new();
        let mut seen = std::collections::HashSet::new();

        if let Some(descriptors) = descriptors {
            for descriptor in descriptors.iter() {
                if let Some(name) = descriptor.family_name() {
                    let name_str = name.to_string();
                    if !seen.contains(&name_str) {
                        seen.insert(name_str.clone());
                        families.push(FontFamily {
                            name: name_str,
                            styles: vec![FontStyle::Normal, FontStyle::Italic],
                        });
                    }
                }
            }
        }

        debug!(count = families.len(), "Enumerated font families");
        Ok(families)
    }

    fn get_metrics(&mut self, descriptor: &FontDescriptor) -> Result<TextMetrics, TextError> {
        let font = self.get_font(descriptor)?;
        Ok(self.get_metrics(font))
    }

    fn get_fallback_fonts(&self, text: &str) -> Vec<String> {
        // Core Text handles font fallback automatically, but we can suggest
        // common fallback chains
        let mut fallbacks = vec![];

        // Check if text contains CJK characters
        let has_cjk = text.chars().any(|c| {
            let cp = c as u32;
            (0x4E00..=0x9FFF).contains(&cp)  // CJK Unified Ideographs
                || (0x3040..=0x309F).contains(&cp)  // Hiragana
                || (0x30A0..=0x30FF).contains(&cp)  // Katakana
                || (0xAC00..=0xD7AF).contains(&cp)  // Hangul
        });

        if has_cjk {
            fallbacks.push("Hiragino Sans".to_string());
            fallbacks.push("Apple SD Gothic Neo".to_string());
            fallbacks.push("PingFang SC".to_string());
        }

        // Check for emoji
        let has_emoji = text.chars().any(|c| {
            let cp = c as u32;
            (0x1F300..=0x1F9FF).contains(&cp)
        });

        if has_emoji {
            fallbacks.push("Apple Color Emoji".to_string());
        }

        // Standard fallbacks
        fallbacks.push("Helvetica Neue".to_string());
        fallbacks.push("Helvetica".to_string());
        fallbacks.push("Arial".to_string());

        fallbacks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_backend() {
        let backend = CoreTextBackend::new();
        assert!(backend.is_ok());
    }
}

