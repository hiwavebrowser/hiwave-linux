//! Image diff tool for screenshot comparison.
//!
//! Compares two PNG images and produces a diff image plus summary statistics.

use std::path::Path;

/// Result of comparing two images.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiffResult {
    /// Total number of pixels compared.
    pub total_pixels: u64,
    /// Number of pixels that differ.
    pub diff_pixels: u64,
    /// Percentage of pixels that differ.
    pub diff_percent: f64,
    /// Maximum per-channel difference found.
    pub max_diff: u8,
    /// Mean per-channel difference.
    pub mean_diff: f64,
    /// Whether the images match within threshold.
    pub matches: bool,
    /// The threshold used for comparison.
    pub threshold: u8,
}

/// Error type for diff operations.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("Image dimensions don't match: {0}x{1} vs {2}x{3}")]
    DimensionMismatch(u32, u32, u32, u32),
    
    #[error("Failed to read image: {0}")]
    ImageRead(String),
    
    #[error("Failed to write image: {0}")]
    ImageWrite(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Load a PNG image as RGBA pixels.
pub fn load_png(path: impl AsRef<Path>) -> Result<(u32, u32, Vec<u8>), DiffError> {
    use std::fs::File;
    
    let file = File::open(path)?;
    let decoder = png::Decoder::new(file);
    let mut reader = decoder.read_info()
        .map_err(|e| DiffError::ImageRead(e.to_string()))?;
    
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)
        .map_err(|e| DiffError::ImageRead(e.to_string()))?;
    
    // Convert to RGBA if needed
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            // Convert RGB to RGBA
            let rgb = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for chunk in rgb.chunks(3) {
                rgba.extend_from_slice(chunk);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::Grayscale => {
            // Convert grayscale to RGBA
            let gray = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for &g in gray {
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
            rgba
        }
        png::ColorType::GrayscaleAlpha => {
            // Convert grayscale+alpha to RGBA
            let ga = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for chunk in ga.chunks(2) {
                let g = chunk[0];
                let a = chunk[1];
                rgba.extend_from_slice(&[g, g, g, a]);
            }
            rgba
        }
        _ => return Err(DiffError::ImageRead("Unsupported color type".into())),
    };
    
    Ok((info.width, info.height, rgba))
}

/// Save RGBA pixels as PNG.
pub fn save_png(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), DiffError> {
    use std::fs::File;
    use std::io::BufWriter;
    
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    
    let mut png_writer = encoder.write_header()
        .map_err(|e| DiffError::ImageWrite(e.to_string()))?;
    png_writer.write_image_data(rgba)
        .map_err(|e| DiffError::ImageWrite(e.to_string()))?;
    
    Ok(())
}

/// Compare two images and produce a diff result.
///
/// # Arguments
/// * `expected` - Path to the expected (golden) image.
/// * `actual` - Path to the actual (test) image.
/// * `threshold` - Per-channel difference threshold (0-255). Pixels with all channels
///   within threshold are considered matching.
///
/// # Returns
/// A `DiffResult` with comparison statistics.
pub fn compare_images(
    expected: impl AsRef<Path>,
    actual: impl AsRef<Path>,
    threshold: u8,
) -> Result<DiffResult, DiffError> {
    let (exp_w, exp_h, exp_data) = load_png(expected)?;
    let (act_w, act_h, act_data) = load_png(actual)?;
    
    if exp_w != act_w || exp_h != act_h {
        return Err(DiffError::DimensionMismatch(exp_w, exp_h, act_w, act_h));
    }
    
    let total_pixels = (exp_w as u64) * (exp_h as u64);
    let mut diff_pixels = 0u64;
    let mut max_diff = 0u8;
    let mut total_diff = 0u64;
    
    for (exp_chunk, act_chunk) in exp_data.chunks(4).zip(act_data.chunks(4)) {
        let mut pixel_matches = true;
        
        for i in 0..4 {
            let diff = (exp_chunk[i] as i16 - act_chunk[i] as i16).unsigned_abs() as u8;
            max_diff = max_diff.max(diff);
            total_diff += diff as u64;
            
            if diff > threshold {
                pixel_matches = false;
            }
        }
        
        if !pixel_matches {
            diff_pixels += 1;
        }
    }
    
    let diff_percent = (diff_pixels as f64 / total_pixels as f64) * 100.0;
    let mean_diff = total_diff as f64 / (total_pixels as f64 * 4.0);
    
    Ok(DiffResult {
        total_pixels,
        diff_pixels,
        diff_percent,
        max_diff,
        mean_diff,
        matches: diff_pixels == 0,
        threshold,
    })
}

/// Compare two images and produce a visual diff image.
///
/// The diff image highlights differences:
/// - Green: pixels that match
/// - Red: pixels that differ
/// - Intensity shows the magnitude of difference
pub fn compare_and_visualize(
    expected: impl AsRef<Path>,
    actual: impl AsRef<Path>,
    diff_output: impl AsRef<Path>,
    threshold: u8,
) -> Result<DiffResult, DiffError> {
    let (exp_w, exp_h, exp_data) = load_png(expected)?;
    let (act_w, act_h, act_data) = load_png(actual)?;
    
    if exp_w != act_w || exp_h != act_h {
        return Err(DiffError::DimensionMismatch(exp_w, exp_h, act_w, act_h));
    }
    
    let total_pixels = (exp_w as u64) * (exp_h as u64);
    let mut diff_pixels = 0u64;
    let mut max_diff = 0u8;
    let mut total_diff = 0u64;
    
    let mut diff_image = Vec::with_capacity(exp_data.len());
    
    for (exp_chunk, act_chunk) in exp_data.chunks(4).zip(act_data.chunks(4)) {
        let mut pixel_matches = true;
        let mut pixel_max_diff = 0u8;
        
        for i in 0..4 {
            let diff = (exp_chunk[i] as i16 - act_chunk[i] as i16).unsigned_abs() as u8;
            max_diff = max_diff.max(diff);
            pixel_max_diff = pixel_max_diff.max(diff);
            total_diff += diff as u64;
            
            if diff > threshold {
                pixel_matches = false;
            }
        }
        
        if pixel_matches {
            // Green for matching pixels (blend with actual)
            diff_image.push(act_chunk[0] / 2);
            diff_image.push((act_chunk[1] / 2).saturating_add(128));
            diff_image.push(act_chunk[2] / 2);
            diff_image.push(255);
        } else {
            diff_pixels += 1;
            // Red intensity based on difference magnitude
            let intensity = ((pixel_max_diff as f32 / 255.0) * 255.0) as u8;
            diff_image.push(intensity.saturating_add(128));
            diff_image.push(act_chunk[1] / 4);
            diff_image.push(act_chunk[2] / 4);
            diff_image.push(255);
        }
    }
    
    let diff_percent = (diff_pixels as f64 / total_pixels as f64) * 100.0;
    let mean_diff = total_diff as f64 / (total_pixels as f64 * 4.0);
    
    // Save diff image
    save_png(diff_output, exp_w, exp_h, &diff_image)?;
    
    Ok(DiffResult {
        total_pixels,
        diff_pixels,
        diff_percent,
        max_diff,
        mean_diff,
        matches: diff_pixels == 0,
        threshold,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    fn create_test_png(width: u32, height: u32, color: [u8; 4]) -> NamedTempFile {
        use std::io::BufWriter;
        
        let file = NamedTempFile::new().unwrap();
        let writer = BufWriter::new(file.reopen().unwrap());
        
        let mut encoder = png::Encoder::new(writer, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        
        let mut png_writer = encoder.write_header().unwrap();
        let data: Vec<u8> = (0..width * height)
            .flat_map(|_| color.iter().copied())
            .collect();
        png_writer.write_image_data(&data).unwrap();
        
        file
    }
    
    #[test]
    fn test_identical_images() {
        let img1 = create_test_png(10, 10, [255, 0, 0, 255]);
        let img2 = create_test_png(10, 10, [255, 0, 0, 255]);
        
        let result = compare_images(img1.path(), img2.path(), 0).unwrap();
        
        assert!(result.matches);
        assert_eq!(result.diff_pixels, 0);
        assert_eq!(result.diff_percent, 0.0);
    }
    
    #[test]
    fn test_different_images() {
        let img1 = create_test_png(10, 10, [255, 0, 0, 255]);
        let img2 = create_test_png(10, 10, [0, 255, 0, 255]);
        
        let result = compare_images(img1.path(), img2.path(), 0).unwrap();
        
        assert!(!result.matches);
        assert_eq!(result.diff_pixels, 100);
        assert_eq!(result.diff_percent, 100.0);
    }
    
    #[test]
    fn test_threshold() {
        let img1 = create_test_png(10, 10, [100, 100, 100, 255]);
        let img2 = create_test_png(10, 10, [105, 105, 105, 255]);
        
        // Should not match with threshold 0
        let result = compare_images(img1.path(), img2.path(), 0).unwrap();
        assert!(!result.matches);
        
        // Should match with threshold 10
        let result = compare_images(img1.path(), img2.path(), 10).unwrap();
        assert!(result.matches);
    }
    
    #[test]
    fn test_dimension_mismatch() {
        let img1 = create_test_png(10, 10, [255, 0, 0, 255]);
        let img2 = create_test_png(20, 20, [255, 0, 0, 255]);
        
        let result = compare_images(img1.path(), img2.path(), 0);
        assert!(matches!(result, Err(DiffError::DimensionMismatch(_, _, _, _))));
    }
}

