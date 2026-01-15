//! Dynamic icon rendering for menu bar.
//!
//! Renders provider icons with usage meters using tiny-skia.
//! Supports both colored mode and template mode (grayscale for macOS).

use exactobar_core::{ProviderKind, StatusIndicator, UsageSnapshot};
use tiny_skia::*;

// ============================================================================
// Constants
// ============================================================================

/// Standard macOS menu bar icon size (18pt at 2x = 36px).
pub const ICON_SIZE: u32 = 36;
pub const ICON_WIDTH: u32 = 36;
pub const ICON_HEIGHT: u32 = 22;

/// Usage bar dimensions.
const BAR_WIDTH: f32 = 24.0;
const BAR_HEIGHT_THICK: f32 = 6.0; // Session bar
const BAR_HEIGHT_THIN: f32 = 2.0; // Weekly bar (hairline)
const BAR_SPACING: f32 = 3.0;
const BAR_RADIUS: f32 = 1.5;

/// Status dot dimensions.
const STATUS_DOT_RADIUS: f32 = 3.0;
const STATUS_DOT_MARGIN: f32 = 2.0;

/// Credits bar (thicker when in credits mode).
const CREDITS_BAR_HEIGHT: f32 = 8.0;

// ============================================================================
// Rendering Mode
// ============================================================================

/// How to render the icon.
#[derive(Debug, Clone, Copy, Default)]
pub enum RenderMode {
    /// Template mode - grayscale, system applies color based on appearance.
    #[default]
    Template,
    /// Colored mode - uses provider brand colors.
    Colored,
}

// ============================================================================
// Icon Renderer
// ============================================================================

/// Renders dynamic menu bar icons with usage meters.
pub struct IconRenderer {
    width: u32,
    height: u32,
    mode: RenderMode,
}

impl Default for IconRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl IconRenderer {
    /// Creates a new icon renderer with standard dimensions.
    pub fn new() -> Self {
        Self {
            width: ICON_WIDTH,
            height: ICON_HEIGHT,
            mode: RenderMode::Template,
        }
    }

    /// Creates a renderer with custom dimensions.
    pub fn with_size(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            mode: RenderMode::Template,
        }
    }

    /// Sets the rendering mode.
    pub fn with_mode(mut self, mode: RenderMode) -> Self {
        self.mode = mode;
        self
    }

    /// Renders an icon for a provider's current usage.
    pub fn render(
        &self,
        provider: ProviderKind,
        snapshot: Option<&UsageSnapshot>,
        stale: bool,
        status: Option<StatusIndicator>,
    ) -> RenderedIcon {
        let mut pixmap = Pixmap::new(self.width, self.height).unwrap();
        pixmap.fill(Color::TRANSPARENT);

        let colors = self.get_colors(provider, stale);

        if let Some(snap) = snapshot {
            self.draw_usage_bars(&mut pixmap, snap, &colors, stale);
        } else {
            self.draw_placeholder(&mut pixmap, &colors);
        }

        // Draw status overlay if there's an incident
        if let Some(indicator) = status {
            if indicator != StatusIndicator::None && indicator != StatusIndicator::Unknown {
                self.draw_status_dot(&mut pixmap, indicator);
            }
        }

        RenderedIcon {
            data: pixmap.data().to_vec(),
            width: self.width,
            height: self.height,
        }
    }

    /// Renders an icon showing credits instead of usage windows.
    pub fn render_credits(
        &self,
        provider: ProviderKind,
        credits_remaining_percent: Option<f64>,
        stale: bool,
    ) -> RenderedIcon {
        let mut pixmap = Pixmap::new(self.width, self.height).unwrap();
        pixmap.fill(Color::TRANSPARENT);

        let colors = self.get_colors(provider, stale);

        if let Some(percent) = credits_remaining_percent {
            self.draw_credits_bar(&mut pixmap, percent as f32, &colors, stale);
        } else {
            self.draw_placeholder(&mut pixmap, &colors);
        }

        RenderedIcon {
            data: pixmap.data().to_vec(),
            width: self.width,
            height: self.height,
        }
    }

    /// Renders a loading animation frame.
    pub fn render_loading(&self, provider: ProviderKind, phase: f64) -> RenderedIcon {
        let mut pixmap = Pixmap::new(self.width, self.height).unwrap();
        pixmap.fill(Color::TRANSPARENT);

        let colors = self.get_colors(provider, false);
        self.draw_loading_animation(&mut pixmap, phase, &colors);

        RenderedIcon {
            data: pixmap.data().to_vec(),
            width: self.width,
            height: self.height,
        }
    }

    /// Renders an error state icon.
    pub fn render_error(&self, provider: ProviderKind) -> RenderedIcon {
        let mut pixmap = Pixmap::new(self.width, self.height).unwrap();
        pixmap.fill(Color::TRANSPARENT);

        self.draw_error_indicator(&mut pixmap, provider);

        RenderedIcon {
            data: pixmap.data().to_vec(),
            width: self.width,
            height: self.height,
        }
    }

    // ========================================================================
    // Color Management
    // ========================================================================

    fn get_colors(&self, provider: ProviderKind, stale: bool) -> IconColors {
        match self.mode {
            RenderMode::Template => IconColors::template(stale),
            RenderMode::Colored => IconColors::colored(provider, stale),
        }
    }

    // ========================================================================
    // Drawing Helpers
    // ========================================================================

    fn draw_usage_bars(
        &self,
        pixmap: &mut Pixmap,
        snapshot: &UsageSnapshot,
        colors: &IconColors,
        stale: bool,
    ) {
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;

        // Calculate bar positions
        let bar_x = center_x - BAR_WIDTH / 2.0;
        let total_height = BAR_HEIGHT_THICK + BAR_SPACING + BAR_HEIGHT_THIN;
        let primary_y = center_y - total_height / 2.0;
        let secondary_y = primary_y + BAR_HEIGHT_THICK + BAR_SPACING;

        // Primary bar (session usage) - thicker
        if let Some(primary) = &snapshot.primary {
            let remaining = 100.0 - primary.used_percent;
            self.draw_bar(
                pixmap,
                bar_x,
                primary_y,
                BAR_WIDTH,
                BAR_HEIGHT_THICK,
                remaining as f32,
                colors,
                stale,
            );
        } else {
            // Draw empty primary bar
            self.draw_empty_bar(pixmap, bar_x, primary_y, BAR_WIDTH, BAR_HEIGHT_THICK, colors);
        }

        // Secondary bar (weekly - hairline)
        if let Some(secondary) = &snapshot.secondary {
            let remaining = 100.0 - secondary.used_percent;
            self.draw_bar(
                pixmap,
                bar_x,
                secondary_y,
                BAR_WIDTH,
                BAR_HEIGHT_THIN,
                remaining as f32,
                colors,
                stale,
            );
        } else {
            // Draw empty secondary bar
            self.draw_empty_bar(pixmap, bar_x, secondary_y, BAR_WIDTH, BAR_HEIGHT_THIN, colors);
        }
    }

    fn draw_bar(
        &self,
        pixmap: &mut Pixmap,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        percent: f32,
        colors: &IconColors,
        stale: bool,
    ) {
        // Background track
        let bg_path = self.rounded_rect_path(x, y, width, height, BAR_RADIUS.min(height / 2.0));
        let bg_paint = create_paint(colors.track);
        pixmap.fill_path(&bg_path, &bg_paint, FillRule::Winding, Transform::identity(), None);

        // Filled portion
        let fill_width = (width * percent / 100.0).max(0.0).min(width);
        if fill_width > 0.0 {
            let fill_path = self.rounded_rect_path(x, y, fill_width, height, BAR_RADIUS.min(height / 2.0));
            let fill_color = if stale {
                colors.fill_stale
            } else {
                self.percent_to_color(percent, colors)
            };
            let fill_paint = create_paint(fill_color);
            pixmap.fill_path(&fill_path, &fill_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    fn draw_empty_bar(
        &self,
        pixmap: &mut Pixmap,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        colors: &IconColors,
    ) {
        let path = self.rounded_rect_path(x, y, width, height, BAR_RADIUS.min(height / 2.0));
        let paint = create_paint(colors.track);
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    fn draw_credits_bar(
        &self,
        pixmap: &mut Pixmap,
        percent: f32,
        colors: &IconColors,
        stale: bool,
    ) {
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;
        let bar_x = center_x - BAR_WIDTH / 2.0;
        let bar_y = center_y - CREDITS_BAR_HEIGHT / 2.0;

        self.draw_bar(pixmap, bar_x, bar_y, BAR_WIDTH, CREDITS_BAR_HEIGHT, percent, colors, stale);
    }

    fn draw_placeholder(&self, pixmap: &mut Pixmap, colors: &IconColors) {
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;
        let bar_x = center_x - BAR_WIDTH / 2.0;

        let total_height = BAR_HEIGHT_THICK + BAR_SPACING + BAR_HEIGHT_THIN;
        let primary_y = center_y - total_height / 2.0;
        let secondary_y = primary_y + BAR_HEIGHT_THICK + BAR_SPACING;

        self.draw_empty_bar(pixmap, bar_x, primary_y, BAR_WIDTH, BAR_HEIGHT_THICK, colors);
        self.draw_empty_bar(pixmap, bar_x, secondary_y, BAR_WIDTH, BAR_HEIGHT_THIN, colors);
    }

    fn draw_loading_animation(&self, pixmap: &mut Pixmap, phase: f64, colors: &IconColors) {
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;
        let bar_x = center_x - BAR_WIDTH / 2.0;

        let total_height = BAR_HEIGHT_THICK + BAR_SPACING + BAR_HEIGHT_THIN;
        let primary_y = center_y - total_height / 2.0;

        // Animated fill using sine wave
        let animated_percent = ((phase.sin() + 1.0) / 2.0 * 100.0) as f32;

        // Draw background
        let bg_path = self.rounded_rect_path(bar_x, primary_y, BAR_WIDTH, BAR_HEIGHT_THICK, BAR_RADIUS);
        let bg_paint = create_paint(colors.track);
        pixmap.fill_path(&bg_path, &bg_paint, FillRule::Winding, Transform::identity(), None);

        // Draw animated fill
        let fill_width = BAR_WIDTH * animated_percent / 100.0;
        if fill_width > 0.0 {
            let fill_path = self.rounded_rect_path(bar_x, primary_y, fill_width, BAR_HEIGHT_THICK, BAR_RADIUS);
            let fill_paint = create_paint(colors.loading);
            pixmap.fill_path(&fill_path, &fill_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    fn draw_error_indicator(&self, pixmap: &mut Pixmap, _provider: ProviderKind) {
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;

        // Draw an X mark
        let size = 8.0;
        let stroke_width = 2.0;

        let color = Color::from_rgba8(255, 59, 48, 204); // Red with ~80% opacity
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;

        // Draw horizontal line of a + (simplified X)
        if let Some(rect) = Rect::from_xywh(
            center_x - size / 2.0,
            center_y - stroke_width / 2.0,
            size,
            stroke_width,
        ) {
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }

        // Draw vertical line
        if let Some(rect) = Rect::from_xywh(
            center_x - stroke_width / 2.0,
            center_y - size / 2.0,
            stroke_width,
            size,
        ) {
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }

    fn draw_status_dot(&self, pixmap: &mut Pixmap, indicator: StatusIndicator) {
        // Position in bottom-right corner
        let x = self.width as f32 - STATUS_DOT_RADIUS - STATUS_DOT_MARGIN;
        let y = self.height as f32 - STATUS_DOT_RADIUS - STATUS_DOT_MARGIN;

        let color = match indicator {
            StatusIndicator::Minor => Color::from_rgba8(255, 193, 7, 255),   // Yellow
            StatusIndicator::Major => Color::from_rgba8(255, 152, 0, 255),   // Orange
            StatusIndicator::Critical => Color::from_rgba8(244, 67, 54, 255), // Red
            _ => return, // Don't draw for None/Unknown
        };

        // Draw circle using path
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, STATUS_DOT_RADIUS);
        if let Some(path) = pb.finish() {
            let paint = create_paint(color);
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    fn rounded_rect_path(&self, x: f32, y: f32, width: f32, height: f32, radius: f32) -> Path {
        let mut pb = PathBuilder::new();

        // For very small radius or small dimensions, just use a regular rectangle
        if radius <= 0.0 || width <= radius * 2.0 || height <= radius * 2.0 {
            pb.push_rect(Rect::from_xywh(x, y, width, height).unwrap());
        } else {
            // Rounded rectangle using arcs
            let r = radius.min(width / 2.0).min(height / 2.0);

            pb.move_to(x + r, y);
            pb.line_to(x + width - r, y);
            pb.quad_to(x + width, y, x + width, y + r);
            pb.line_to(x + width, y + height - r);
            pb.quad_to(x + width, y + height, x + width - r, y + height);
            pb.line_to(x + r, y + height);
            pb.quad_to(x, y + height, x, y + height - r);
            pb.line_to(x, y + r);
            pb.quad_to(x, y, x + r, y);
            pb.close();
        }

        pb.finish().unwrap()
    }

    fn percent_to_color(&self, percent: f32, colors: &IconColors) -> Color {
        if percent > 50.0 {
            colors.good
        } else if percent > 20.0 {
            colors.warning
        } else {
            colors.danger
        }
    }
}

// ============================================================================
// Icon Colors
// ============================================================================

/// Color palette for icon rendering.
struct IconColors {
    track: Color,       // Bar background
    fill_stale: Color,  // Fill when stale
    good: Color,        // >50% remaining
    warning: Color,     // 20-50% remaining
    danger: Color,      // <20% remaining
    loading: Color,     // Loading animation
}

impl IconColors {
    /// Template mode colors (grayscale for macOS template images).
    fn template(stale: bool) -> Self {
        let opacity = if stale { 140 } else { 204 }; // ~55% or ~80%

        Self {
            track: Color::from_rgba8(0, 0, 0, 102),        // 40% opacity
            fill_stale: Color::from_rgba8(0, 0, 0, 140),   // 55% opacity
            good: Color::from_rgba8(0, 0, 0, opacity),     // 80% or 55%
            warning: Color::from_rgba8(0, 0, 0, opacity),  // Same for template
            danger: Color::from_rgba8(0, 0, 0, opacity),   // Same for template
            loading: Color::from_rgba8(0, 0, 0, 128),      // 50% opacity
        }
    }

    /// Colored mode with provider brand colors.
    fn colored(provider: ProviderKind, stale: bool) -> Self {
        let brand = provider_brand_color(provider);
        let alpha_mult = if stale { 0.7 } else { 1.0 };

        Self {
            track: Color::from_rgba8(80, 80, 80, 180),
            fill_stale: with_alpha(brand, 0.6),
            good: with_alpha(brand, alpha_mult),
            warning: with_alpha(Color::from_rgba8(255, 193, 7, 255), alpha_mult),
            danger: with_alpha(Color::from_rgba8(244, 67, 54, 255), alpha_mult),
            loading: Color::from_rgba8(150, 150, 150, 200),
        }
    }
}

/// Gets the brand color for a provider.
fn provider_brand_color(provider: ProviderKind) -> Color {
    match provider {
        ProviderKind::Codex => Color::from_rgba8(16, 163, 127, 255),    // OpenAI green
        ProviderKind::Claude => Color::from_rgba8(204, 119, 68, 255),   // Anthropic orange/tan
        ProviderKind::Cursor => Color::from_rgba8(138, 92, 246, 255),   // Purple
        ProviderKind::Gemini => Color::from_rgba8(66, 133, 244, 255),   // Google blue
        ProviderKind::Copilot => Color::from_rgba8(36, 41, 46, 255),    // GitHub dark
        ProviderKind::Factory => Color::from_rgba8(255, 107, 107, 255), // Red
        ProviderKind::VertexAI => Color::from_rgba8(66, 133, 244, 255), // Google blue
        ProviderKind::Zai => Color::from_rgba8(100, 100, 100, 255),     // Gray
        ProviderKind::Augment => Color::from_rgba8(75, 0, 130, 255),    // Indigo
        ProviderKind::Kiro => Color::from_rgba8(255, 165, 0, 255),      // Orange
        ProviderKind::MiniMax => Color::from_rgba8(0, 191, 255, 255),   // Deep sky blue
        ProviderKind::Antigravity => Color::from_rgba8(148, 0, 211, 255), // Violet
    }
}

/// Adjusts color alpha.
fn with_alpha(color: Color, alpha: f64) -> Color {
    Color::from_rgba(
        color.red(),
        color.green(),
        color.blue(),
        (color.alpha() as f64 * alpha) as f32,
    )
    .unwrap_or(color)
}

/// Creates a paint with the given color.
fn create_paint(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    paint
}

// ============================================================================
// Rendered Icon
// ============================================================================

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
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use exactobar_core::UsageWindow;

    #[test]
    fn test_render_empty() {
        let renderer = IconRenderer::new();
        let icon = renderer.render(ProviderKind::Codex, None, false, None);

        assert_eq!(icon.width, ICON_WIDTH);
        assert_eq!(icon.height, ICON_HEIGHT);
        assert!(!icon.data.is_empty());
    }

    #[test]
    fn test_render_with_snapshot() {
        let renderer = IconRenderer::new();

        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(25.0));
        snapshot.secondary = Some(UsageWindow::new(50.0));

        let icon = renderer.render(ProviderKind::Claude, Some(&snapshot), false, None);
        assert!(!icon.data.is_empty());
    }

    #[test]
    fn test_render_stale() {
        let renderer = IconRenderer::new();
        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(25.0));

        let icon = renderer.render(ProviderKind::Claude, Some(&snapshot), true, None);
        assert!(!icon.data.is_empty());
    }

    #[test]
    fn test_render_with_status() {
        let renderer = IconRenderer::new();
        let icon = renderer.render(
            ProviderKind::Codex,
            None,
            false,
            Some(StatusIndicator::Minor),
        );
        assert!(!icon.data.is_empty());
    }

    #[test]
    fn test_render_credits() {
        let renderer = IconRenderer::new();
        let icon = renderer.render_credits(ProviderKind::Codex, Some(75.0), false);
        assert!(!icon.data.is_empty());
    }

    #[test]
    fn test_render_loading() {
        let renderer = IconRenderer::new();
        let icon = renderer.render_loading(ProviderKind::Codex, 0.5);
        assert!(!icon.data.is_empty());
    }

    #[test]
    fn test_render_error() {
        let renderer = IconRenderer::new();
        let icon = renderer.render_error(ProviderKind::Codex);
        assert!(!icon.data.is_empty());
    }

    #[test]
    fn test_to_png() {
        let renderer = IconRenderer::new();
        let icon = renderer.render(ProviderKind::Codex, None, false, None);
        let png = icon.to_png();

        // PNG magic bytes
        assert!(png.starts_with(&[0x89, 0x50, 0x4E, 0x47]));
    }

    #[test]
    fn test_colored_mode() {
        let renderer = IconRenderer::new().with_mode(RenderMode::Colored);
        let icon = renderer.render(ProviderKind::Claude, None, false, None);
        assert!(!icon.data.is_empty());
    }

    #[test]
    fn test_template_mode() {
        let renderer = IconRenderer::new().with_mode(RenderMode::Template);
        let icon = renderer.render(ProviderKind::Claude, None, false, None);
        assert!(!icon.data.is_empty());
    }
}
