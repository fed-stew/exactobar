//! Loading spinner component.

use gpui::*;

/// Animated loading spinner.
pub struct Spinner {
    size: Pixels,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spinner {
    pub fn new() -> Self {
        Self { size: px(16.0) }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }
}

impl IntoElement for Spinner {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        div()
            .w(self.size)
            .h(self.size)
            .flex()
            .items_center()
            .justify_center()
            .text_color(hsla(0.0, 0.0, 0.5, 1.0))
            .text_size(self.size * 0.8)
            .child("◌")
    }
}
