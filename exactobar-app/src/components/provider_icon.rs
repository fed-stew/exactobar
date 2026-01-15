//! Provider icon component.

use exactobar_core::ProviderKind;
use gpui::*;

/// Provider icon with brand color.
pub struct ProviderIcon {
    provider: ProviderKind,
    size: Pixels,
}

impl ProviderIcon {
    pub fn new(provider: ProviderKind) -> Self {
        Self {
            provider,
            size: px(24.0),
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    fn brand_color(&self) -> Hsla {
        match self.provider {
            ProviderKind::Codex => hsla(160.0 / 360.0, 0.82, 0.35, 1.0),
            ProviderKind::Claude => hsla(25.0 / 360.0, 0.55, 0.53, 1.0),
            ProviderKind::Cursor => hsla(265.0 / 360.0, 0.70, 0.60, 1.0),
            ProviderKind::Gemini => hsla(217.0 / 360.0, 0.91, 0.60, 1.0),
            ProviderKind::Copilot => hsla(215.0 / 360.0, 0.14, 0.34, 1.0),
            ProviderKind::Factory => hsla(0.0, 0.70, 0.60, 1.0),
            ProviderKind::VertexAI => hsla(217.0 / 360.0, 0.91, 0.60, 1.0),
            ProviderKind::Zai => hsla(0.0, 0.0, 0.40, 1.0),
            ProviderKind::Augment => hsla(275.0 / 360.0, 1.0, 0.25, 1.0),
            ProviderKind::Kiro => hsla(39.0 / 360.0, 1.0, 0.50, 1.0),
            ProviderKind::MiniMax => hsla(195.0 / 360.0, 1.0, 0.50, 1.0),
            ProviderKind::Antigravity => hsla(282.0 / 360.0, 1.0, 0.41, 1.0),
        }
    }

    fn icon_char(&self) -> &'static str {
        match self.provider {
            ProviderKind::Codex => "O",
            ProviderKind::Claude => "C",
            ProviderKind::Cursor => "⌘",
            ProviderKind::Gemini => "G",
            ProviderKind::Copilot => "⌥",
            ProviderKind::Factory => "D",
            ProviderKind::VertexAI => "V",
            ProviderKind::Zai => "Z",
            ProviderKind::Augment => "A",
            ProviderKind::Kiro => "K",
            ProviderKind::MiniMax => "M",
            ProviderKind::Antigravity => "∞",
        }
    }
}

impl IntoElement for ProviderIcon {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let color = self.brand_color();

        div()
            .w(self.size)
            .h(self.size)
            .rounded(px(4.0))
            .bg(color)
            .flex()
            .items_center()
            .justify_center()
            .text_color(white())
            .text_size(self.size * 0.5)
            .font_weight(FontWeight::BOLD)
            .child(self.icon_char())
    }
}
