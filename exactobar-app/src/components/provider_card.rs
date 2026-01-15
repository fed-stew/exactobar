//! Provider card component.

use exactobar_core::ProviderKind;
use exactobar_providers::ProviderRegistry;
use gpui::*;

use crate::components::{ProviderIcon, Spinner, UsageBar};
use crate::state::AppState;

/// Full provider card with usage display.
pub struct ProviderCard {
    provider: ProviderKind,
    snapshot: Option<exactobar_core::UsageSnapshot>,
    is_refreshing: bool,
    error: Option<String>,
    session_label: &'static str,
    weekly_label: &'static str,
    name: String,
}

impl ProviderCard {
    pub fn new(provider: ProviderKind, cx: &App) -> Self {
        let state = cx.global::<AppState>();
        let snapshot = state.get_snapshot(provider, cx);
        let is_refreshing = state.is_provider_refreshing(provider, cx);
        let error = state.get_error(provider, cx);
        let descriptor = ProviderRegistry::get(provider);

        let name = descriptor
            .map(|d| d.display_name().to_string())
            .unwrap_or_else(|| format!("{:?}", provider));
        let session_label = descriptor
            .map(|d| d.metadata.session_label.as_str())
            .unwrap_or("Session");
        let weekly_label = descriptor
            .map(|d| d.metadata.weekly_label.as_str())
            .unwrap_or("Weekly");

        Self {
            provider,
            snapshot,
            is_refreshing,
            error,
            session_label,
            weekly_label,
            name,
        }
    }
}

impl IntoElement for ProviderCard {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let surface_color = hsla(0.0, 0.0, 0.98, 1.0);
        let border_color = hsla(0.0, 0.0, 0.9, 1.0);
        let muted_color = hsla(0.0, 0.0, 0.5, 1.0);
        let error_bg = hsla(0.0, 0.5, 0.95, 1.0);
        let error_text = hsla(0.0, 0.7, 0.5, 1.0);

        let mut card = div()
            .p(px(12.0))
            .bg(surface_color)
            .rounded(px(8.0))
            .border_1()
            .border_color(border_color)
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(ProviderIcon::new(self.provider))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.name.clone()),
                            ),
                    )
                    .child(if self.is_refreshing {
                        Spinner::new().into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            );

        // Error state
        if let Some(err) = &self.error {
            card = card.child(
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .rounded(px(4.0))
                    .bg(error_bg)
                    .text_sm()
                    .text_color(error_text)
                    .child(err.clone()),
            );
        }

        // Usage content
        if self.error.is_none() {
            if let Some(snap) = &self.snapshot {
                let mut usage_div = div()
                    .mt(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0));

                if let Some(primary) = &snap.primary {
                    let remaining = 100.0 - primary.used_percent;
                    usage_div = usage_div.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(div().w(px(60.0)).text_sm().child(self.session_label))
                            .child(UsageBar::new(remaining as f32).flex_1())
                            .child(
                                div()
                                    .w(px(45.0))
                                    .text_sm()
                                    .text_right()
                                    .child(format!("{:.0}%", remaining)),
                            ),
                    );

                    if let Some(desc) = &primary.reset_description {
                        usage_div = usage_div.child(
                            div()
                                .text_xs()
                                .text_color(muted_color)
                                .child(format!("Resets {}", desc)),
                        );
                    }
                }

                if let Some(secondary) = &snap.secondary {
                    let remaining = 100.0 - secondary.used_percent;
                    usage_div = usage_div.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(div().w(px(60.0)).text_sm().child(self.weekly_label))
                            .child(UsageBar::new(remaining as f32).height(px(4.0)).flex_1())
                            .child(
                                div()
                                    .w(px(45.0))
                                    .text_sm()
                                    .text_right()
                                    .child(format!("{:.0}%", remaining)),
                            ),
                    );
                }

                card = card.child(usage_div);
            } else if !self.is_refreshing {
                card = card.child(
                    div()
                        .mt(px(12.0))
                        .text_sm()
                        .text_color(muted_color)
                        .child("No usage data available"),
                );
            }
        }

        card
    }
}
