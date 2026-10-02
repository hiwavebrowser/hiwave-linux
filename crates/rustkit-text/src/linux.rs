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
    FontDescriptor, FontStyle, FontWeight, GlyphInfo, ShapedGlyph, ShapedText,
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
    /// Candidate name -> installed family (None = not installed).
    family_cache: HashMap<String, Option<String>>,
    /// `webfonts::generation()` the caches were filled under; a change means a
    /// family name may now resolve to a different face.
    webfont_generation: u64,
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
            family_cache: HashMap::new(),
            webfont_generation: crate::webfonts::generation(),
        })
    }

    fn sync_webfonts(&mut self) {
        let generation = crate::webfonts::generation();
        if generation != self.webfont_generation {
            self.face_cache.clear();
            self.family_cache.clear();
            self.webfont_generation = generation;
        }
    }

    /// Find a font file matching the descriptor.
    fn find_font(&self, descriptor: &FontDescriptor) -> Result<PathBuf, TextError> {
        // The fontconfig style name for the requested weight/slant. The old
        // code built a pattern string and then discarded it, so bold and
        // italic text resolved to the regular face.
        let bold = descriptor.weight.0 >= 600;
        let style = match (bold, descriptor.style) {
            (true, FontStyle::Normal) => "Bold",
            (true, _) => "Bold Italic",
            (false, FontStyle::Normal) => "Regular",
            (false, _) => "Italic",
        };

        self.fontconfig
            .find(&descriptor.family, Some(style))
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

        self.sync_webfonts();
        if !self.face_cache.contains_key(&key) {
            let italic = !matches!(descriptor.style, FontStyle::Normal);
            let web = crate::webfonts::lookup_data(&descriptor.family, descriptor.weight.0 as u16, italic);
            let face = match web {
                Some(data) => self
                    .ft_library
                    .new_memory_face((*data).clone(), 0)
                    .map_err(|e| TextError::FontNotFound(format!("Failed to load web font: {:?}", e)))?,
                None => {
                    let path = self.find_font(descriptor)?;
                    self.ft_library
                        .new_face(&path, 0)
                        .map_err(|e| TextError::FontNotFound(format!("Failed to load font: {:?}", e)))?
                }
            };
            self.face_cache.insert(key.clone(), face);
        }

        // The cache key omits the size (one face per family/weight/style), so
        // the size must be applied on every fetch: applying it only at load
        // pinned every later request to whichever size asked first.
        let face = self.face_cache.get(&key).unwrap();
        face.set_char_size(
            (descriptor.size * 64.0) as isize, // width in 1/64 points
            (descriptor.size * 64.0) as isize, // height in 1/64 points
            72,                                 // horizontal DPI
            72,                                 // vertical DPI
        )
        .map_err(|e| TextError::ShapingFailed(format!("Failed to set char size: {:?}", e)))?;

        Ok(face)
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
            advance: glyph.linear_hori_advance() as f32 / 65536.0,
            ascent,
        })
    }
}

impl LinuxTextBackend {
    /// First candidate that names an installed family (or a CSS generic that
    /// fontconfig resolves itself), else `sans-serif`. Layout and the renderer
    /// both resolve through this so the face that is measured is the face that
    /// is painted.
    pub fn resolve_family<'a>(&mut self, candidates: impl IntoIterator<Item = &'a str>) -> String {
        self.sync_webfonts();
        for raw in candidates {
            let name = raw.trim().trim_matches(|c| c == '"' || c == '\'');
            if name.is_empty() {
                continue;
            }
            // A face the document registered via @font-face outranks every
            // platform lookup — the family may exist nowhere else.
            if crate::webfonts::is_installed(name) {
                return name.to_string();
            }
            if let Some(hit) = self.family_cache.get(name) {
                if let Some(hit) = hit {
                    return hit.clone();
                }
                continue;
            }
            let lower = name.to_ascii_lowercase();
            let resolved = if matches!(lower.as_str(), "sans-serif" | "serif" | "monospace") {
                Some(lower.clone())
            } else {
                // Metric-compatible substitutes (Arial -> Liberation Sans ...)
                // are real matches: fontconfig aliases them on purpose and
                // Chrome on Linux resolves them the same way.
                let metric_alias = matches!(
                    lower.as_str(),
                    "arial" | "helvetica" | "times new roman" | "times" | "courier new" | "courier"
                );
                self.fontconfig
                    .find(name, None)
                    .filter(|f| metric_alias || f.name.eq_ignore_ascii_case(name))
                    .map(|_| name.to_string())
            };
            self.family_cache.insert(name.to_string(), resolved.clone());
            if let Some(r) = resolved {
                return r;
            }
        }
        "sans-serif".to_string()
    }

    /// (ascent, descent, line gap) in pixels, each rounded the way Blink
    /// rounds them before summing into a line box.
    pub fn line_metrics(&mut self, descriptor: &FontDescriptor) -> Result<(f32, f32, f32), TextError> {
        let face = self.get_face(descriptor)?;
        let upem = face.em_size() as f32;
        if upem <= 0.0 {
            return Err(TextError::ShapingFailed("face reports no units_per_EM".into()));
        }
        let scale = descriptor.size / upem;
        let ascent = (face.ascender() as f32 * scale).round();
        let descent = (face.descender() as f32 * scale).abs().round();
        let gap = ((face.height() as f32 - (face.ascender() as f32 - face.descender() as f32)) * scale)
            .max(0.0)
            .round();
        Ok((ascent, descent, gap))
    }

    /// Unhinted horizontal advance of each char, in pixels, at the
    /// descriptor's size (subpixel positioning, as Chrome lays out).
    pub fn advance_widths(
        &mut self,
        text: &str,
        descriptor: &FontDescriptor,
    ) -> Result<Vec<f32>, TextError> {
        let face = self.get_face(descriptor)?;
        let mut out = Vec::with_capacity(text.len());
        for c in text.chars() {
            face.load_char(c as usize, freetype::face::LoadFlag::NO_HINTING | freetype::face::LoadFlag::NO_BITMAP)
                .map_err(|e| TextError::ShapingFailed(format!("load_char {c:?}: {e:?}")))?;
            out.push(face.glyph().linear_hori_advance() as f32 / 65536.0);
        }
        Ok(out)
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

    fn get_font_families(&self) -> Result<Vec<String>, TextError> {
        // Fontconfig can enumerate everything; the common-family list is all
        // any caller has ever needed (currently: none outside tests).
        Ok(vec![
            "DejaVu Sans".to_string(),
            "Liberation Sans".to_string(),
            "Noto Sans".to_string(),
            "Ubuntu".to_string(),
            "FreeSans".to_string(),
        ])
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
    use super::*;

    fn desc(family: &str, size: f32, weight: u32) -> FontDescriptor {
        FontDescriptor {
            family: family.to_string(),
            weight: crate::FontWeight(weight),
            style: FontStyle::Normal,
            size,
        }
    }

    #[test]
    fn advances_follow_the_requested_size_not_the_first_one() {
        let mut b = LinuxTextBackend::new().unwrap();
        let small: f32 = b.advance_widths("Hello", &desc("sans-serif", 10.0, 400)).unwrap().iter().sum();
        let large: f32 = b.advance_widths("Hello", &desc("sans-serif", 20.0, 400)).unwrap().iter().sum();
        let back: f32 = b.advance_widths("Hello", &desc("sans-serif", 10.0, 400)).unwrap().iter().sum();
        assert!((large / small - 2.0).abs() < 0.01, "20px must be twice 10px, got {small} vs {large}");
        assert!((back - small).abs() < 0.001, "returning to 10px must restore 10px widths");
    }

    #[test]
    fn a_registered_web_font_is_measured_from_its_own_bytes() {
        const AHEM: &[u8] = include_bytes!("../tests/fixtures/Ahem.ttf");
        let mut backend = LinuxTextBackend::new().expect("backend");
        let family = "WebfontsLinuxAhemProbe";
        assert_eq!(backend.resolve_family([family]), "sans-serif", "not installed yet");

        crate::webfonts::install(
            "linux-probe",
            &[crate::webfonts::WebFontFace {
                family: family.to_string(),
                weight: 400,
                italic: false,
                data: std::sync::Arc::new(AHEM.to_vec()),
            }],
        );
        // Second call after the install: the stale "not installed" entry must
        // not be served from the family cache.
        assert_eq!(backend.resolve_family([family]), family);
        let descriptor = FontDescriptor {
            family: family.to_string(),
            weight: FontWeight(400),
            style: FontStyle::Normal,
            size: 20.0,
        };
        let adv = backend.advance_widths("XX", &descriptor).expect("web face measures");
        assert!(
            adv.len() == 2 && adv.iter().all(|a| (a - 20.0).abs() < 0.01),
            "Ahem advances are 1em (16.16 fixed): {adv:?}"
        );
        crate::webfonts::clear();
        assert_eq!(backend.resolve_family([family]), "sans-serif", "cleared set is forgotten");
    }

    #[test]
    fn resolve_family_skips_uninstalled_names_and_keeps_generics() {
        let mut b = LinuxTextBackend::new().unwrap();
        assert_eq!(b.resolve_family(["No Such Family Zzz", "monospace"]), "monospace");
        assert_eq!(b.resolve_family(["No Such Family Zzz"]), "sans-serif");
        assert_eq!(b.resolve_family([" \"serif\" "]), "serif");
    }
}

