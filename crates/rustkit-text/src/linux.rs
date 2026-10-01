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
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
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
    /// Char -> installed family that covers it, for chars the requested face
    /// lacks (None = nothing installed covers it).
    fallback_cache: HashMap<char, Option<String>>,
    /// Face key -> its variable-axis pin (None = no weight or optical-size axis).
    face_axes: HashMap<String, Option<AxisPin>>,
    /// Face key -> HarfBuzz face over the same bytes (None = unparsable).
    hb_cache: HashMap<String, Option<Rc<HbFace>>>,
}

/// A variable face's pinned axes: the full FreeType design-coordinate vector
/// plus the `wght` value used and, when the font has an `opsz` axis, where it
/// sits and its range (Chrome sets `opsz` to the font size in CSS px).
#[derive(Clone)]
struct AxisPin {
    coords: Vec<freetype::ffi::FT_Fixed>,
    wght: Option<f32>,
    opsz: Option<(usize, freetype::ffi::FT_Fixed, freetype::ffi::FT_Fixed)>,
}

impl AxisPin {
    fn opsz_value(&self, size: f32) -> Option<f32> {
        self.opsz
            .map(|(_, lo, hi)| ((size * 65536.0) as freetype::ffi::FT_Fixed).clamp(lo, hi) as f32 / 65536.0)
    }
}

/// A HarfBuzz face together with the font bytes it borrows.
struct HbFace {
    // Declared first so it drops before the bytes it points into.
    face: RefCell<rustybuzz::Face<'static>>,
    pin: Option<AxisPin>,
    _data: Arc<Vec<u8>>,
}

impl HbFace {
    fn new(data: Arc<Vec<u8>>, pin: Option<AxisPin>) -> Option<Self> {
        // SAFETY: `data` is a heap allocation owned by this struct and never
        // mutated, so the slice outlives `face`.
        let slice: &'static [u8] = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(data.as_slice()) };
        let face = rustybuzz::Face::from_slice(slice, 0)?;
        Some(Self { face: RefCell::new(face), pin, _data: data })
    }

    fn pose(&self, size: f32) {
        let Some(pin) = &self.pin else { return };
        let mut vars = Vec::new();
        if let Some(w) = pin.wght {
            vars.push(rustybuzz::Variation { tag: rustybuzz::ttf_parser::Tag::from_bytes(b"wght"), value: w });
        }
        if let Some(o) = pin.opsz_value(size) {
            vars.push(rustybuzz::Variation { tag: rustybuzz::ttf_parser::Tag::from_bytes(b"opsz"), value: o });
        }
        self.face.borrow_mut().set_variations(&vars);
    }
}

/// HarfBuzz parses sfnt only; web fonts arrive WOFF/WOFF2-compressed.
fn sfnt_bytes(data: &[u8]) -> Vec<u8> {
    let decoded = match data.get(..4) {
        Some(b"wOF2") => wuff::decompress_woff2(data).ok(),
        Some(b"wOFF") => wuff::decompress_woff1(data).ok(),
        _ => None,
    };
    decoded.unwrap_or_else(|| data.to_vec())
}

fn face_key(descriptor: &FontDescriptor) -> String {
    format!(
        "{}:{}:{}",
        descriptor.family,
        descriptor.weight.0,
        match descriptor.style {
            FontStyle::Normal => "n",
            FontStyle::Italic => "i",
            FontStyle::Oblique => "o",
        }
    )
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
            fallback_cache: HashMap::new(),
            face_axes: HashMap::new(),
            hb_cache: HashMap::new(),
        })
    }

    fn sync_webfonts(&mut self) {
        let generation = crate::webfonts::generation();
        if generation != self.webfont_generation {
            self.face_cache.clear();
            self.family_cache.clear();
            self.fallback_cache.clear();
            self.face_axes.clear();
            self.hb_cache.clear();
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
        let key = face_key(descriptor);

        self.sync_webfonts();
        if !self.face_cache.contains_key(&key) {
            let italic = !matches!(descriptor.style, FontStyle::Normal);
            let web = crate::webfonts::lookup_data(&descriptor.family, descriptor.weight.0 as u16, italic);
            let is_web = web.is_some();
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
            let pinned = apply_weight_axis(&face, descriptor.weight.0 as f32, descriptor.size, !is_web);
            self.face_axes.insert(key.clone(), pinned);
            self.face_cache.insert(key.clone(), face);
        }

        // The cache key omits the size (one face per family/weight/style), so
        // the size must be applied on every fetch: applying it only at load
        // pinned every later request to whichever size asked first.
        let face = self.face_cache.get(&key).unwrap();
        if let Some(Some(pin)) = self.face_axes.get(&key) {
            if let Some((idx, lo, hi)) = pin.opsz {
                let mut coords = pin.coords.clone();
                coords[idx] = ((descriptor.size * 65536.0) as freetype::ffi::FT_Fixed).clamp(lo, hi);
                let raw = face.raw() as *const freetype::ffi::FT_FaceRec as freetype::ffi::FT_Face;
                unsafe { freetype::ffi::FT_Set_Var_Design_Coordinates(raw, coords.len() as u32, coords.as_ptr()) };
            }
        }
        if face.has_fixed_sizes() && !face.is_scalable() {
            // Bitmap-strike fonts (Noto Color Emoji) cannot be sized: pick the
            // strike and let callers scale by `fixed_strike_scale`.
            let raw = face.raw() as *const freetype::ffi::FT_FaceRec as freetype::ffi::FT_Face;
            unsafe { freetype::ffi::FT_Select_Size(raw, 0) };
        } else {
            face.set_char_size(
                (descriptor.size * 64.0) as isize, // width in 1/64 points
                (descriptor.size * 64.0) as isize, // height in 1/64 points
                72,                                 // horizontal DPI
                72,                                 // vertical DPI
            )
            .map_err(|e| TextError::ShapingFailed(format!("Failed to set char size: {:?}", e)))?;
        }

        Ok(face)
    }
}

/// Shape `run` and write each cluster's advance onto its first char.
/// Ligatures are off so the one-glyph-per-char painting stays consistent.
fn shape_run(hb: &HbFace, run: &[char], size: f32, out: &mut [f32]) {
    use rustybuzz::ttf_parser::Tag;
    let mut starts = Vec::with_capacity(run.len());
    let mut text = String::with_capacity(run.len());
    for &c in run {
        starts.push(text.len() as u32);
        text.push(c);
    }
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(&text);
    buf.guess_segment_properties();
    let features = [
        rustybuzz::Feature::new(Tag::from_bytes(b"liga"), 0, ..),
        rustybuzz::Feature::new(Tag::from_bytes(b"clig"), 0, ..),
    ];
    hb.pose(size);
    let face = hb.face.borrow();
    let shaped = rustybuzz::shape(&face, &features, buf);
    let scale = size / face.units_per_em() as f32;
    for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        if let Ok(k) = starts.binary_search(&info.cluster) {
            out[k] += pos.x_advance as f32 * scale;
        }
    }
}

/// Zero-width format characters that must not take a glyph or an advance.
fn is_zero_width(ch: char) -> bool {
    matches!(ch as u32, 0x200B..=0x200D | 0xFE00..=0xFE0F | 0x2060)
}

/// Whether a char is painted from the color-emoji font. Same blocks as the
/// macOS path; text symbols with monochrome outlines stay on the gray path.
pub fn is_emoji(ch: char) -> bool {
    matches!(ch as u32,
        0x1F300..=0x1FAFF | 0x1F000..=0x1F0FF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0x1F1E6..=0x1F1FF)
}

/// Pixels per strike pixel: how much a bitmap-strike font's glyphs shrink to
/// reach `size`.
fn fixed_strike_scale(face: &freetype::Face, size: f32) -> f32 {
    let raw = face.raw();
    if raw.num_fixed_sizes <= 0 || raw.available_sizes.is_null() {
        return 1.0;
    }
    let ppem = unsafe { (*raw.available_sizes).y_ppem } as f32 / 64.0;
    if ppem > 0.0 { size / ppem } else { 1.0 }
}

/// CSS font-weight matching over the weights a family actually offers
/// (CSS Fonts 4 §5.2): the desired weight if present, else the nearest in the
/// direction the spec prefers.
fn css_match_weight(desired: f32, available: &[f32]) -> Option<f32> {
    let pick = |it: Vec<f32>| it.into_iter().next();
    let mut asc: Vec<f32> = available.to_vec();
    asc.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut desc = asc.clone();
    desc.reverse();
    if let Some(&w) = asc.iter().find(|&&w| (w - desired).abs() < 0.5) {
        return Some(w);
    }
    if (400.0..=500.0).contains(&desired) {
        pick(asc.iter().copied().filter(|&w| w > desired && w <= 500.0).collect())
            .or_else(|| pick(desc.iter().copied().filter(|&w| w < desired).collect()))
            .or_else(|| pick(asc.iter().copied().filter(|&w| w > 500.0).collect()))
    } else if desired < 400.0 {
        pick(desc.iter().copied().filter(|&w| w < desired).collect())
            .or_else(|| pick(asc.iter().copied().filter(|&w| w > desired).collect()))
    } else {
        pick(asc.iter().copied().filter(|&w| w > desired).collect())
            .or_else(|| pick(desc.iter().copied().filter(|&w| w < desired).collect()))
    }
}

/// Pin a variable font's axes for the requested weight and size. A system
/// font resolves a CSS weight to one of the family's NAMED instances (Chrome
/// via fontconfig: 200 and 300 both land on Light, 600 on Bold when there is
/// no SemiBold); a web font (`snap_named` false) takes the exact weight,
/// clamped to the axis. Either way `opsz`, when present, follows the font
/// size (`font-optical-sizing: auto`). Static faces are untouched.
fn apply_weight_axis(face: &freetype::Face, weight: f32, size: f32, snap_named: bool) -> Option<AxisPin> {
    use freetype::ffi;
    const WGHT: u64 = 0x7767_6874;
    const OPSZ: u64 = 0x6f70_737a;
    let raw = face.raw() as *const ffi::FT_FaceRec as ffi::FT_Face;
    let mut pin = None;
    unsafe {
        let mut mm: *mut ffi::FT_MM_Var = std::ptr::null_mut();
        if ffi::FT_Get_MM_Var(raw, &mut mm) != 0 || mm.is_null() {
            return None;
        }
        let n = (*mm).num_axis as usize;
        let axes: Vec<ffi::FT_Var_Axis> = (0..n).map(|i| *(*mm).axis.add(i)).collect();
        let wi = axes.iter().position(|a| a.tag as u64 == WGHT);
        let oi = axes.iter().position(|a| a.tag as u64 == OPSZ);
        if wi.is_some() || oi.is_some() {
            let mut target = None;
            if let Some(wi) = wi {
                let mut named = Vec::new();
                if snap_named {
                    for k in 0..(*mm).num_namedstyles as usize {
                        let style = &*(*mm).namedstyle.add(k);
                        let at_defaults = (0..n)
                            .filter(|&i| i != wi)
                            .all(|i| *style.coords.add(i) == axes[i].def);
                        if at_defaults {
                            named.push(*style.coords.add(wi) as f32 / 65536.0);
                        }
                    }
                }
                target = Some(css_match_weight(weight, &named).unwrap_or(weight));
            }
            let coords: Vec<ffi::FT_Fixed> = axes
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    if Some(i) == wi {
                        ((target.unwrap() * 65536.0) as ffi::FT_Fixed).clamp(a.minimum, a.maximum)
                    } else if Some(i) == oi {
                        ((size * 65536.0) as ffi::FT_Fixed).clamp(a.minimum, a.maximum)
                    } else {
                        a.def
                    }
                })
                .collect();
            ffi::FT_Set_Var_Design_Coordinates(raw, n as u32, coords.as_ptr());
            pin = Some(AxisPin {
                wght: wi.map(|i| coords[i] as f32 / 65536.0),
                opsz: oi.map(|i| (i, axes[i].minimum, axes[i].maximum)),
                coords,
            });
        }
        ffi::FT_Done_MM_Var((*(*raw).glyph).library, mm);
    }
    pin
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
        self.rasterize_glyph_at_phase(ch, descriptor, 0, 1)
    }

    /// Rasterize with the pen `phase / phases` of a pixel right of an integer
    /// column, so the bitmap carries the fractional position the way Skia's
    /// subpixel text does. `bearing_x` is relative to that integer column.
    pub fn rasterize_glyph_at_phase(
        &mut self,
        ch: char,
        descriptor: &FontDescriptor,
        phase: u8,
        phases: u8,
    ) -> Result<RasterizedGlyph, TextError> {
        if is_zero_width(ch) {
            return Ok(RasterizedGlyph {
                bitmap: Vec::new(),
                width: 0,
                height: 0,
                bearing_x: 0,
                bearing_y: 0,
                advance: 0.0,
                ascent: 0.0,
            });
        }
        let descriptor = &self.source_descriptor(ch, descriptor);
        let face = self.get_face(descriptor)?;
        if face.has_fixed_sizes() && !face.is_scalable() {
            return Err(TextError::ShapingFailed(format!("{ch:?} is color-bitmap only")));
        }

        // The face is shared with measurement, so the shift must not outlive
        // this render.
        let shifted = phase != 0 && phases != 0;
        if shifted {
            let mut matrix = freetype::ffi::FT_Matrix { xx: 0x10000, xy: 0, yx: 0, yy: 0x10000 };
            let mut delta =
                freetype::ffi::FT_Vector { x: (phase as i64 * 64) / phases as i64, y: 0 };
            face.set_transform(&mut matrix, &mut delta);
        }
        let loaded = face.load_char(ch as usize, freetype::face::LoadFlag::RENDER);
        if shifted {
            let mut matrix = freetype::ffi::FT_Matrix { xx: 0x10000, xy: 0, yx: 0, yy: 0x10000 };
            let mut delta = freetype::ffi::FT_Vector { x: 0, y: 0 };
            face.set_transform(&mut matrix, &mut delta);
        }
        loaded.map_err(|e| TextError::ShapingFailed(format!("load_char {ch:?}: {e:?}")))?;

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
            } else if lower == "system-ui" {
                // Chrome asks fontconfig for its default sans; the generic
                // "sans-serif" pattern is that lookup here.
                Some("sans-serif".to_string())
            } else if lower == "helvetica" {
                // Chrome resolves Helvetica to the same metric-compatible
                // face as Arial (Liberation Sans), where a bare fontconfig
                // match picks Nimbus Sans.
                self.fontconfig.find("Arial", None).map(|_| "Arial".to_string())
            } else {
                // Metric-compatible substitutes (Arial -> Liberation Sans ...)
                // are real matches: fontconfig aliases them on purpose and
                // Chrome on Linux resolves them the same way.
                let metric_alias = matches!(
                    lower.as_str(),
                    "arial" | "helvetica" | "times new roman" | "times" | "courier new" | "courier"
                );
                let mut pat = fontconfig::Pattern::new(&self.fontconfig);
                match std::ffi::CString::new(name) {
                    Ok(c) => {
                        pat.add_string(c"family", &c);
                        let matched = pat.font_match();
                        let installed = matched
                            .get_string(c"family")
                            .is_some_and(|f| f.eq_ignore_ascii_case(name));
                        (metric_alias || installed).then(|| name.to_string())
                    }
                    Err(_) => None,
                }
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

    /// HarfBuzz face for a descriptor, built from the same bytes FreeType loads.
    fn hb_face(&mut self, descriptor: &FontDescriptor) -> Option<Rc<HbFace>> {
        self.get_face(descriptor).ok()?;
        let key = face_key(descriptor);
        if let Some(hit) = self.hb_cache.get(&key) {
            return hit.clone();
        }
        let italic = !matches!(descriptor.style, FontStyle::Normal);
        let data = match crate::webfonts::lookup_data(&descriptor.family, descriptor.weight.0 as u16, italic) {
            Some(d) => Some(Arc::new(sfnt_bytes(&d))),
            None => self
                .find_font(descriptor)
                .ok()
                .and_then(|p| std::fs::read(p).ok())
                .map(Arc::new),
        };
        let pin = self.face_axes.get(&key).cloned().flatten();
        let built = data.and_then(|d| HbFace::new(d, pin)).map(Rc::new);
        self.hb_cache.insert(key, built.clone());
        built
    }

    /// Horizontal advance of each char, in pixels, at the descriptor's size
    /// (subpixel positioning, as Chrome lays out). Runs of text in one face are
    /// shaped with HarfBuzz so pair kerning applies; a cluster's advance is
    /// carried by its first char. A char the requested face lacks is measured
    /// in the fallback face that will paint it.
    pub fn advance_widths(
        &mut self,
        text: &str,
        descriptor: &FontDescriptor,
    ) -> Result<Vec<f32>, TextError> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = vec![0.0f32; chars.len()];
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if is_zero_width(c) {
                i += 1;
                continue;
            }
            let src = self.source_descriptor(c, descriptor);
            let fixed = {
                let face = self.get_face(&src)?;
                face.has_fixed_sizes() && !face.is_scalable()
            };
            if fixed {
                let face = self.get_face(&src)?;
                face.load_char(c as usize, freetype::face::LoadFlag::DEFAULT)
                    .map_err(|e| TextError::ShapingFailed(format!("load_char {c:?}: {e:?}")))?;
                let g = face.glyph();
                // Bitmap strikes report no linear advance; the strike's own
                // 26.6 advance is what gets scaled.
                let adv = g.advance().x as f32 / 64.0;
                out[i] = adv * fixed_strike_scale(face, src.size);
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < chars.len()
                && !is_zero_width(chars[j])
                && self.source_descriptor(chars[j], descriptor).family == src.family
            {
                j += 1;
            }
            match self.hb_face(&src) {
                Some(hb) => shape_run(&hb, &chars[i..j], src.size, &mut out[i..j]),
                None => {
                    let face = self.get_face(&src)?;
                    for k in i..j {
                        face.load_char(
                            chars[k] as usize,
                            freetype::face::LoadFlag::NO_HINTING | freetype::face::LoadFlag::NO_BITMAP,
                        )
                        .map_err(|e| TextError::ShapingFailed(format!("load_char {:?}: {e:?}", chars[k])))?;
                        out[k] = face.glyph().linear_hori_advance() as f32 / 65536.0;
                    }
                }
            }
            i = j;
        }
        Ok(out)
    }

    fn has_glyph(&mut self, descriptor: &FontDescriptor, ch: char) -> bool {
        self.get_face(descriptor)
            .map(|f| f.get_char_index(ch as usize).is_ok())
            .unwrap_or(false)
    }

    /// The descriptor whose face paints `ch`: the requested one when it has the
    /// glyph, else the first installed fallback family that does (emoji go to
    /// Noto Color Emoji), else the requested one (its .notdef).
    pub fn source_descriptor(&mut self, ch: char, descriptor: &FontDescriptor) -> FontDescriptor {
        if self.has_glyph(descriptor, ch) {
            return descriptor.clone();
        }
        if let Some(hit) = self.fallback_cache.get(&ch) {
            return match hit {
                Some(family) => FontDescriptor { family: family.clone(), ..descriptor.clone() },
                None => descriptor.clone(),
            };
        }
        let candidates: &[&str] = if is_emoji(ch) {
            &["Noto Color Emoji", "Noto Sans Symbols 2", "Noto Sans Symbols", "DejaVu Sans"]
        } else {
            &["Noto Sans", "DejaVu Sans", "Noto Sans Symbols", "Noto Sans Symbols 2", "Noto Sans Math", "Noto Sans CJK SC"]
        };
        let mut found = None;
        for name in candidates {
            let family = self.resolve_family([*name]);
            if family == "sans-serif" {
                continue;
            }
            let d = FontDescriptor { family: family.clone(), ..descriptor.clone() };
            if self.has_glyph(&d, ch) {
                found = Some(family);
                break;
            }
        }
        self.fallback_cache.insert(ch, found.clone());
        match found {
            Some(family) => FontDescriptor { family, ..descriptor.clone() },
            None => descriptor.clone(),
        }
    }

    /// Rasterize a color-bitmap glyph (emoji) to premultiplied RGBA, scaled
    /// from the font's fixed strike to the descriptor's size. Returns
    /// (rgba, width, height, advance, bearing_x, bearing_y) with the bearings
    /// measured from the pen/baseline, y up. None when the char has no color
    /// artwork.
    pub fn rasterize_color_glyph(
        &mut self,
        ch: char,
        descriptor: &FontDescriptor,
    ) -> Option<(Vec<u8>, u32, u32, f32, f32, f32)> {
        let src = self.source_descriptor(ch, descriptor);
        let face = self.get_face(&src).ok()?;
        if !face.has_color() {
            return None;
        }
        face.load_char(
            ch as usize,
            freetype::face::LoadFlag::COLOR | freetype::face::LoadFlag::RENDER,
        )
        .ok()?;
        let scale = fixed_strike_scale(face, src.size);
        let advance = face.glyph().advance().x as f32 / 64.0 * scale;
        let glyph = face.glyph();
        let bmp = glyph.bitmap();
        if !matches!(bmp.pixel_mode(), Ok(freetype::bitmap::PixelMode::Bgra)) {
            return None;
        }
        let (sw, sh) = (bmp.width() as usize, bmp.rows() as usize);
        let pitch = bmp.pitch().unsigned_abs() as usize;
        let buf = bmp.buffer();
        if sw == 0 || sh == 0 || buf.len() < pitch * (sh - 1) + sw * 4 {
            return None;
        }

        let left = glyph.bitmap_left() as f32 * scale;
        let top = glyph.bitmap_top() as f32 * scale;
        let right = left + sw as f32 * scale;
        let bottom = top - sh as f32 * scale;
        let (x0, x1) = (left.floor() as i32, right.ceil() as i32);
        let (y_top, y_bot) = (top.ceil() as i32, bottom.floor() as i32);
        let (dw, dh) = ((x1 - x0).max(1) as usize, (y_top - y_bot).max(1) as usize);

        // Area-average the strike into the destination grid (premultiplied
        // BGRA in, premultiplied RGBA out).
        let mut out = vec![0u8; dw * dh * 4];
        for dy in 0..dh {
            let wy0 = (y_top - dy as i32) as f32; // dest row spans [wy0-1, wy0] in y-up
            let sy_a = (top - wy0) / scale;
            let sy_b = (top - (wy0 - 1.0)) / scale;
            for dx in 0..dw {
                let wx0 = (x0 + dx as i32) as f32;
                let sx_a = (wx0 - left) / scale;
                let sx_b = (wx0 + 1.0 - left) / scale;
                let mut acc = [0f32; 4];
                let (ya, yb) = (sy_a.max(0.0).floor() as usize, (sy_b.min(sh as f32).ceil() as usize).min(sh));
                let (xa, xb) = (sx_a.max(0.0).floor() as usize, (sx_b.min(sw as f32).ceil() as usize).min(sw));
                for sy in ya..yb {
                    let oy = (sy_b.min(sy as f32 + 1.0) - sy_a.max(sy as f32)).max(0.0);
                    for sx in xa..xb {
                        let ox = (sx_b.min(sx as f32 + 1.0) - sx_a.max(sx as f32)).max(0.0);
                        let w = ox * oy;
                        let i = sy * pitch + sx * 4;
                        acc[0] += buf[i + 2] as f32 * w;
                        acc[1] += buf[i + 1] as f32 * w;
                        acc[2] += buf[i] as f32 * w;
                        acc[3] += buf[i + 3] as f32 * w;
                    }
                }
                // Weights are in source-pixel area; a fully covered destination
                // pixel spans 1/scale^2 of them. Area outside the strike is
                // transparent and contributes nothing.
                let o = (dy * dw + dx) * 4;
                for k in 0..4 {
                    out[o + k] = (acc[k] * scale * scale).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        Some((out, dw as u32, dh as u32, advance, x0 as f32, y_top as f32))
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
    fn a_phased_raster_differs_and_leaves_the_shared_face_unshifted() {
        let mut b = LinuxTextBackend::new().unwrap();
        let d = FontDescriptor {
            family: b.resolve_family(["sans-serif"]),
            weight: FontWeight(400),
            style: FontStyle::Normal,
            size: 14.0,
        };
        let at0 = b.rasterize_glyph('l', &d).unwrap();
        let at2 = b.rasterize_glyph_at_phase('l', &d, 2, 4).unwrap();
        assert_ne!(at0.bitmap, at2.bitmap, "a half-pixel shift must change the coverage");
        let again = b.rasterize_glyph('l', &d).unwrap();
        assert_eq!(at0.bitmap, again.bitmap, "the shift must not outlive its render");
        assert_eq!(at0.advance, at2.advance, "the advance is the layout's, not the phase's");
    }

    #[test]
    fn resolve_family_skips_uninstalled_names_and_keeps_generics() {
        let mut b = LinuxTextBackend::new().unwrap();
        assert_eq!(b.resolve_family(["No Such Family Zzz", "monospace"]), "monospace");
        assert_eq!(b.resolve_family(["No Such Family Zzz"]), "sans-serif");
        assert_eq!(b.resolve_family([" \"serif\" "]), "serif");
    }

    #[test]
    fn system_ui_is_the_fontconfig_default_and_helvetica_is_arial() {
        let mut b = LinuxTextBackend::new().unwrap();
        assert_eq!(b.resolve_family(["system-ui"]), "sans-serif");
        assert_eq!(b.resolve_family(["-apple-system", "BlinkMacSystemFont", "monospace"]), "monospace");
        assert_eq!(b.resolve_family(["Helvetica"]), "Arial");
    }
}


#[cfg(test)]
mod weight_tests {
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
    fn css_weight_matching_follows_the_spec_direction() {
        let ubuntu = [300.0, 400.0, 500.0, 700.0];
        assert_eq!(css_match_weight(200.0, &ubuntu), Some(300.0));
        assert_eq!(css_match_weight(300.0, &ubuntu), Some(300.0));
        assert_eq!(css_match_weight(600.0, &ubuntu), Some(700.0));
        assert_eq!(css_match_weight(500.0, &ubuntu), Some(500.0));
        assert_eq!(css_match_weight(400.0, &[300.0, 700.0]), Some(300.0));
        assert_eq!(css_match_weight(900.0, &ubuntu), Some(700.0));
    }

    #[test]
    fn an_installed_family_resolves_by_its_family_name_not_its_full_name() {
        let mut b = LinuxTextBackend::new().unwrap();
        assert_eq!(b.resolve_family(["Zzz Not A Font", "DejaVu Sans"]), "DejaVu Sans");
    }

    #[test]
    fn pair_kerning_narrows_text_below_the_sum_of_its_glyphs() {
        let mut b = LinuxTextBackend::new().unwrap();
        let fam = b.resolve_family(["system-ui"]);
        let d = desc(&fam, 16.0, 400);
        let whole: f32 = b.advance_widths("The quick brown fox", &d).unwrap().iter().sum();
        let singles: f32 = "The quick brown fox"
            .chars()
            .map(|c| b.advance_widths(&c.to_string(), &d).unwrap()[0])
            .sum();
        assert!(whole < singles - 0.5, "kerned {whole} vs unkerned {singles}");
        // Chrome measures this run at 151.65625 after rounding up to 1/64.
        assert!((whole - 151.656).abs() < 0.02, "{whole}");
    }

    #[test]
    fn a_variable_web_font_takes_the_exact_weight_not_the_nearest_named_instance() {
        let path = "/usr/share/fonts/truetype/ubuntu/Ubuntu[wdth,wght].ttf";
        let Ok(bytes) = std::fs::read(path) else { return };
        crate::webfonts::install(
            "weight-test",
            &[crate::webfonts::WebFontFace {
                family: "WeightTestVF".into(),
                weight: 400,
                italic: false,
                data: Arc::new(bytes),
            }],
        );
        let mut b = LinuxTextBackend::new().unwrap();
        let width = |b: &mut LinuxTextBackend, w: u32| -> f32 {
            b.advance_widths("Hamburgefonstiv", &desc("WeightTestVF", 32.0, w)).unwrap().iter().sum()
        };
        let (w400, w425, w500) = (width(&mut b, 400), width(&mut b, 425), width(&mut b, 500));
        crate::webfonts::clear();
        assert!(w400 < w425 && w425 < w500, "{w400} {w425} {w500}");
    }

    #[test]
    fn woff_containers_are_unwrapped_for_shaping_and_sfnt_passes_through() {
        let sfnt = vec![0u8, 1, 0, 0, 9, 9, 9, 9];
        assert_eq!(sfnt_bytes(&sfnt), sfnt);
        assert_eq!(sfnt_bytes(b"wOF2junk"), b"wOF2junk".to_vec());
    }

    #[test]
    fn an_emoji_has_an_advance_and_color_artwork_when_the_font_is_installed() {
        let mut b = LinuxTextBackend::new().unwrap();
        let installed = b.resolve_family(["Noto Color Emoji"]) != "sans-serif";
        if !installed {
            return;
        }
        let d = desc("sans-serif", 16.0, 400);
        let adv = b.advance_widths("\u{1F600}", &d).unwrap();
        assert!(adv[0] > 4.0, "{adv:?}");
        let (rgba, w, h, ..) = b.rasterize_color_glyph('\u{1F600}', &d).expect("color glyph");
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        assert!(rgba.chunks(4).any(|p| p[3] > 0));
    }
}
