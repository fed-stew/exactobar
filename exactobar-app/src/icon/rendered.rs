//! Rendered icon output.
//!
//! This module contains the [`RenderedIcon`] struct which holds
//! the rendered RGBA pixel data and provides conversion methods.

/// A rendered icon as RGBA pixel data.
pub struct RenderedIcon {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl RenderedIcon {
    /// Converts to PNG bytes.
    pub fn to_png(&self) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};

        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(self.width, self.height, self.data.clone())
                .expect("Failed to create image buffer");

        let mut png_data = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_data);
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .expect("Failed to encode PNG");

        png_data
    }

    /// Gets the bytes as BGRA (for some platform APIs).
    pub fn to_bgra(&self) -> Vec<u8> {
        self.data
            .chunks(4)
            .flat_map(|rgba| [rgba[2], rgba[1], rgba[0], rgba[3]])
            .collect()
    }

    /// Gets raw RGBA pixels with dimensions.
    ///
    /// Returns (width, height, pixels) tuple for use with platform APIs
    /// that need direct pixel access (like Linux SNI).
    pub fn to_rgba_pixels(&self) -> (u32, u32, Vec<u8>) {
        (self.width, self.height, self.data.clone())
    }
}
