//! About pane.

use gpui::*;

use super::SettingsTheme;

/// About settings pane.
pub struct AboutPane {
    theme: SettingsTheme,
}

impl AboutPane {
    pub fn new(theme: SettingsTheme) -> Self {
        Self { theme }
    }
}

impl IntoElement for AboutPane {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let theme = self.theme;
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .items_center()
            .pt(px(40.0))
            .pb(px(24.0))
            .child(
                div()
                    .w(px(80.0))
                    .h(px(80.0))
                    .rounded(px(16.0))
                    .bg(theme.brand)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(white())
                    .text_size(px(40.0))
                    .font_weight(FontWeight::BOLD)
                    .child("E"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child("ExactoBar"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .text_center()
                    .max_w(px(350.0))
                    .child("A macOS menu bar app for monitoring LLM provider usage. Built with GPUI."),
            )
            .child(
                div()
                    .flex()
                    .gap(px(16.0))
                    .mt(px(16.0))
                    .child(render_link("GitHub", theme))
                    .child(render_link("Report Issue", theme)),
            )
            .child(
                div()
                    .mt(px(40.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child("Built with"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(12.0))
                            .child(render_tech_badge("Rust", theme))
                            .child(render_tech_badge("GPUI", theme))
                            .child(render_tech_badge("Tokio", theme)),
                    ),
            )
            .child(
                div()
                    .mt(px(40.0))
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child("© 2025 ExactoBar Contributors"),
            )
    }
}

fn render_link(label: &'static str, theme: SettingsTheme) -> Div {
    div()
        .text_sm()
        .text_color(theme.link)
        .cursor_pointer()
        .hover(|s| s.underline())
        .child(label)
}

fn render_tech_badge(name: &'static str, theme: SettingsTheme) -> Div {
    div()
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .bg(theme.code_bg)
        .text_xs()
        .child(name)
}
