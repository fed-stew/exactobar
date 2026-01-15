//! Advanced settings pane.

use gpui::*;

use crate::components::Toggle;
use crate::state::AppState;
use super::SettingsTheme;

/// Advanced settings pane.
pub struct AdvancedPane {
    debug_mode: bool,
    auto_refresh_on_wake: bool,
    status_checks_enabled: bool,
    session_quota_notifications_enabled: bool,
    cost_usage_enabled: bool,
    random_blink_enabled: bool,
    claude_web_extras_enabled: bool,
    show_optional_credits_and_extra_usage: bool,
    openai_web_access_enabled: bool,
    theme: SettingsTheme,
}

impl AdvancedPane {
    pub fn new<V: 'static>(cx: &Context<V>, theme: SettingsTheme) -> Self {
        let state = cx.global::<AppState>();
        let settings = state.settings.read(cx).settings();
        Self {
            debug_mode: settings.debug_mode,
            auto_refresh_on_wake: settings.auto_refresh_on_wake,
            status_checks_enabled: settings.status_checks_enabled,
            session_quota_notifications_enabled: settings.session_quota_notifications_enabled,
            cost_usage_enabled: settings.cost_usage_enabled,
            random_blink_enabled: settings.random_blink_enabled,
            claude_web_extras_enabled: settings.claude_web_extras_enabled,
            show_optional_credits_and_extra_usage: settings.show_optional_credits_and_extra_usage,
            openai_web_access_enabled: settings.openai_web_access_enabled,
            theme,
        }
    }
}

impl IntoElement for AdvancedPane {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let config_dir = exactobar_store::default_config_dir();
        let cache_dir = exactobar_store::default_cache_dir();
        let theme = self.theme;

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .pb(px(24.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .child("Advanced"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child("Advanced configuration options"),
                    ),
            )
            // Debug Mode
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Debug Mode"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Enable verbose logging for troubleshooting"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-debug-mode")
                            .checked(self.debug_mode)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_debug_mode(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // Auto-refresh on Wake
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Auto-refresh on Wake"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Refresh usage data when your Mac wakes from sleep"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-auto-refresh-on-wake")
                            .checked(self.auto_refresh_on_wake)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_auto_refresh_on_wake(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // Status Page Checks
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Status Page Checks"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Check provider status pages for outages"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-status-checks")
                            .checked(self.status_checks_enabled)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_status_checks_enabled(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // Quota Notifications
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Quota Notifications"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Notify when approaching quota limits"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-quota-notifications")
                            .checked(self.session_quota_notifications_enabled)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_session_quota_notifications_enabled(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // Cost Tracking
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Cost Tracking"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Track provider costs from local usage logs"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-cost-tracking")
                            .checked(self.cost_usage_enabled)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_cost_usage_enabled(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // Random Blink
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Random Blink"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Enable random blink animation on status icon"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-random-blink")
                            .checked(self.random_blink_enabled)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_random_blink_enabled(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // Claude Web Extras
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Claude Web Extras"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Fetch extra Claude usage via browser cookies"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-claude-web-extras")
                            .checked(self.claude_web_extras_enabled)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_claude_web_extras_enabled(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // Show Credits & Extras
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Show Credits & Extras"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Show credits and extra usage in menu"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-show-credits-extras")
                            .checked(self.show_optional_credits_and_extra_usage)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_show_optional_credits_and_extra_usage(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // OpenAI Web Access
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("OpenAI Web Access"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Enable OpenAI dashboard access for Codex"),
                            ),
                    )
                    .child(
                        Toggle::new("toggle-openai-web-access")
                            .checked(self.openai_web_access_enabled)
                            .on_toggle(|enabled, cx| {
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.settings.update(cx, |model, _| {
                                        model.set_openai_web_access_enabled(enabled);
                                    });
                                });
                            }),
                    ),
            )
            // Paths section
            .child(
                div()
                    .mt(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Data Locations"),
                    )
                    .child(
                        div()
                            .p(px(12.0))
                            .rounded(px(8.0))
                            .bg(theme.code_bg)
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child("Config Directory"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .child(config_dir.display().to_string()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child("Cache Directory"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .child(cache_dir.display().to_string()),
                                    ),
                            ),
                    ),
            )
    }
}
