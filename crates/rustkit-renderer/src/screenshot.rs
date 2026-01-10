//! Screenshot capture via GPU readback.
//!
//! Provides functionality to capture rendered frames to PNG files
//! for testing and debugging purposes.

use std::path::Path;

/// Error type for screenshot operations.
#[derive(Debug, thiserror::Error)]
pub enum ScreenshotError {
    #[error("Buffer mapping failed")]
    BufferMapFailed,
    
    #[error("PNG encoding failed: {0}")]
    PngEncoding(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Screenshot metadata for test verification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenshotMetadata {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// GPU adapter name.
    pub adapter: String,
    /// Texture format used.
    pub format: String,
    /// Timestamp of capture.
    pub timestamp: String,
    /// Number of color vertices rendered.
    pub color_vertex_count: usize,
    /// Number of texture vertices rendered (text/images).
    pub texture_vertex_count: usize,
}

/// GPU readback buffer for capturing rendered frames.
pub struct GpuReadbackBuffer {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    bytes_per_row: u32,
}

impl GpuReadbackBuffer {
    /// Create a new readback buffer for the given dimensions.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // RGBA8 = 4 bytes per pixel, aligned to 256 bytes
        let bytes_per_row = (width * 4 + 255) & !255;
        let buffer_size = bytes_per_row * height;
        
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screenshot Readback Buffer"),
            size: buffer_size as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        
        Self {
            buffer,
            width,
            height,
            bytes_per_row,
        }
    }
    
    /// Copy from a texture to this readback buffer.
    pub fn copy_from_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }
    
    /// Read the buffer data (blocking).
    pub async fn read_data(&self, device: &wgpu::Device) -> Result<Vec<u8>, ScreenshotError> {
        let buffer_slice = self.buffer.slice(..);
        
        let (tx, rx) = tokio::sync::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        
        device.poll(wgpu::Maintain::Wait);
        
        rx.await
            .map_err(|_| ScreenshotError::BufferMapFailed)?
            .map_err(|_| ScreenshotError::BufferMapFailed)?;
        
        let data = buffer_slice.get_mapped_range();
        
        // Remove row padding
        let mut result = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height {
            let start = (y * self.bytes_per_row) as usize;
            let end = start + (self.width * 4) as usize;
            result.extend_from_slice(&data[start..end]);
        }
        
        drop(data);
        self.buffer.unmap();
        
        Ok(result)
    }
    
    /// Read buffer data synchronously (blocks current thread).
    pub fn read_data_sync(&self, device: &wgpu::Device) -> Result<Vec<u8>, ScreenshotError> {
        let buffer_slice = self.buffer.slice(..);
        
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        
        device.poll(wgpu::Maintain::Wait);
        
        rx.recv()
            .map_err(|_| ScreenshotError::BufferMapFailed)?
            .map_err(|_| ScreenshotError::BufferMapFailed)?;
        
        let data = buffer_slice.get_mapped_range();
        
        // Remove row padding
        let mut result = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height {
            let start = (y * self.bytes_per_row) as usize;
            let end = start + (self.width * 4) as usize;
            result.extend_from_slice(&data[start..end]);
        }
        
        drop(data);
        self.buffer.unmap();
        
        Ok(result)
    }
}

/// Save RGBA pixel data as PNG.
pub fn save_png(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    rgba_data: &[u8],
) -> Result<(), ScreenshotError> {
    use std::fs::File;
    use std::io::BufWriter;
    
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    
    let mut png_writer = encoder
        .write_header()
        .map_err(|e| ScreenshotError::PngEncoding(e.to_string()))?;
    
    png_writer
        .write_image_data(rgba_data)
        .map_err(|e| ScreenshotError::PngEncoding(e.to_string()))?;
    
    Ok(())
}

/// Save screenshot metadata as JSON.
pub fn save_metadata(
    path: impl AsRef<Path>,
    metadata: &ScreenshotMetadata,
) -> Result<(), ScreenshotError> {
    let json = serde_json::to_string_pretty(metadata)
        .map_err(|e| ScreenshotError::PngEncoding(e.to_string()))?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Create an offscreen render target for screenshot capture.
pub fn create_offscreen_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Offscreen Screenshot Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    
    (texture, view)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metadata_serialization() {
        let metadata = ScreenshotMetadata {
            width: 800,
            height: 600,
            adapter: "Test Adapter".to_string(),
            format: "Rgba8UnormSrgb".to_string(),
            timestamp: "2025-01-04T12:00:00Z".to_string(),
            color_vertex_count: 100,
            texture_vertex_count: 50,
        };
        
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("800"));
        assert!(json.contains("Test Adapter"));
    }
}

