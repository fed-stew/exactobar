//! Settings window.

mod about;
mod advanced;
mod general;
mod providers;
mod theme;

use gpui::prelude::*;
use gpui::*;

use exactobar_core::ProviderKind;
use exactobar_store::{CookieSource, DataSourceMode};

use about::AboutPane;
use advanced::AdvancedPane;
use general::GeneralPane;
use providers::{
    COOKIE_SOURCES, DATA_SOURCE_MODES, ProviderRowData, ProviderStatus, collect_provider_data,
    get_install_command, prompt_for_api_key_async,
};
pub use theme::SettingsTheme;

use crate::components::ProviderIcon;
use crate::state::AppState;

// ============================================================================
// Settings Window
// ============================================================================

/// The main settings window.
pub struct SettingsWindow {
    active_pane: SettingsPane,
    settings_subscription: Option<gpui::Subscription>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SettingsPane {
    #[default]
    General,
    Providers,
    Advanced,
    About,
}

impl SettingsWindow {
    pub fn new() -> Self {
        println!("🎯 [SW-1] SettingsWindow::new() called!");
        let result = Self {
            active_pane: SettingsPane::default(),
            settings_subscription: None,
        };
        println!("🎯 [SW-2] SettingsWindow::new() returning!");
        result
    }
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        println!(
            "🎨 [RENDER] SettingsWindow::render() called! active_pane={:?}",
            self.active_pane
        );

        if self.settings_subscription.is_none() {
            let settings = cx.global::<AppState>().settings.clone();
            self.settings_subscription = Some(cx.observe(&settings, |_this, _model, cx| {
                cx.notify();
            }));
        }

        // Get theme mode from settings
        let theme_mode = cx.global::<AppState>().settings.read(cx).theme_mode();

        // Determine theme based on user's preference
        let theme = match theme_mode {
            exactobar_store::ThemeMode::Dark => SettingsTheme::dark(),
            exactobar_store::ThemeMode::Light => SettingsTheme::light(),
            exactobar_store::ThemeMode::System => {
                let is_dark = matches!(
                    window.appearance(),
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                );
                if is_dark {
                    SettingsTheme::dark()
                } else {
                    SettingsTheme::light()
                }
            }
        };

        let active = self.active_pane;

        let content = match self.active_pane {
            SettingsPane::General => GeneralPane::new(cx, theme).into_any_element(),
            SettingsPane::Providers => self.render_providers_pane(cx, theme).into_any_element(),
            SettingsPane::Advanced => AdvancedPane::new(cx, theme).into_any_element(),
            SettingsPane::About => AboutPane::new(theme).into_any_element(),
        };

        // Build sidebar items with click handlers inline
        let sidebar = div()
            .w(px(180.0))
            .h_full()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .p(px(8.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .text_color(theme.text_primary)
            .child(self.sidebar_item(
                SettingsPane::General,
                "General",
                "⚙",
                active == SettingsPane::General,
                &theme,
                cx,
            ))
            .child(self.sidebar_item(
                SettingsPane::Providers,
                "Providers",
                "◉",
                active == SettingsPane::Providers,
                &theme,
                cx,
            ))
            .child(self.sidebar_item(
                SettingsPane::Advanced,
                "Advanced",
                "⌘",
                active == SettingsPane::Advanced,
                &theme,
                cx,
            ))
            .child(self.sidebar_item(
                SettingsPane::About,
                "About",
                "ℹ",
                active == SettingsPane::About,
                &theme,
                cx,
            ));

        div()
            .size_full()
            .flex()
            .bg(theme.bg)
            .text_color(theme.text_primary)
            .child(sidebar)
            .child(
                div()
                    .id("settings-content-scroll")
                    .flex_1()
                    .h_full()
                    .overflow_y_scroll()
                    .child(div().p(px(24.0)).child(content)),
            )
    }
}

impl SettingsWindow {
    /// Renders the providers pane with proper cx.listener() click handlers.
    fn render_providers_pane(
        &self,
        cx: &mut Context<Self>,
        theme: SettingsTheme,
    ) -> impl IntoElement {
        let providers = collect_provider_data(cx);

        // Separate primary and additional providers
        let (primary, additional): (Vec<_>, Vec<_>) =
            providers.into_iter().partition(|p| p.is_primary);

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
                            .child("Providers"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child("Enable the LLM providers you want to monitor"),
                    ),
            )
            // Primary Providers section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_muted)
                            .child("Primary Providers"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .children(
                                primary
                                    .into_iter()
                                    .map(|data| self.render_provider_row(data, theme, cx)),
                            ),
                    ),
            )
            // Additional Providers section
            .when(!additional.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_muted)
                                .child("Additional Providers"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .rounded(px(8.0))
                                .border_1()
                                .border_color(theme.border)
                                .overflow_hidden()
                                .children(
                                    additional
                                        .into_iter()
                                        .map(|data| self.render_provider_row(data, theme, cx)),
                                ),
                        ),
                )
            })
    }

    /// Renders a provider row with toggle and settings.
    fn render_provider_row(
        &self,
        data: ProviderRowData,
        theme: SettingsTheme,
        cx: &mut Context<Self>,
    ) -> Div {
        let provider = data.provider;
        let hover_bg = theme.hover;
        let has_settings = data.supports_cookies || data.supports_data_source;
        let is_enabled = data.is_enabled;

        // Toggle colors
        let track_color = if is_enabled {
            hsla(217.0 / 360.0, 0.91, 0.60, 1.0) // Blue when checked
        } else {
            hsla(0.0, 0.0, 0.8, 1.0) // Gray when unchecked
        };
        let knob_offset = if is_enabled { px(14.0) } else { px(2.0) };

        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(theme.border)
            // Main row
            .child(
                div()
                    .px(px(16.0))
                    .py(px(12.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .hover(move |s| s.bg(hover_bg))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(ProviderIcon::new(provider))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(data.name.clone()),
                                            )
                                            .when(data.is_primary, |el| {
                                                el.child(
                                                    div()
                                                        .text_xs()
                                                        .px(px(6.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(theme.selected)
                                                        .child("Primary"),
                                                )
                                            })
                                            // Status indicator
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(data.status.color())
                                                    .child(data.status.indicator()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child(format!("CLI: {}", data.cli_name)),
                                    ),
                            ),
                    )
                    // Toggle switch with cx.listener()!
                    .child(
                        div()
                            .id(SharedString::from(format!("toggle-{:?}", provider)))
                            .w(px(32.0))
                            .h(px(18.0))
                            .rounded(px(9.0))
                            .bg(track_color)
                            .border_1()
                            .border_color(hsla(0.0, 0.0, 0.7, 1.0))
                            .relative()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_this, _, _window, cx| {
                                    println!("🎯 [TOGGLE] Provider {:?} clicked!", provider);
                                    cx.update_global::<AppState, _>(|state, cx| {
                                        let enabling =
                                            !state.settings.read(cx).is_provider_enabled(provider);
                                        state.settings.update(cx, |model, _| {
                                            model.toggle_provider(provider);
                                        });
                                        if enabling {
                                            state.refresh_provider(provider, cx);
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(1.0))
                                    .left(knob_offset)
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .rounded_full()
                                    .bg(white())
                                    .shadow_sm(),
                            ),
                    ),
            )
            // Settings row (only show when enabled and provider has settings)
            .when(has_settings && is_enabled, |el| {
                el.child(
                    div()
                        .px(px(16.0))
                        .pb(px(12.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        // Cookie source selector
                        .when(data.supports_cookies, |el| {
                            el.child(self.render_cookie_source_selector(
                                provider,
                                data.current_cookie_source,
                                theme,
                                cx,
                            ))
                        })
                        // Data source selector
                        .when(data.supports_data_source, |el| {
                            el.child(self.render_data_source_selector(
                                provider,
                                data.current_data_source.unwrap_or(DataSourceMode::Auto),
                                theme,
                                cx,
                            ))
                        }),
                )
            })
            // Install hint (only show when enabled but CLI is missing)
            .when(
                is_enabled && matches!(data.status, ProviderStatus::CliMissing),
                |el| {
                    let cli_name = data.cli_name.clone();
                    let install_cmd = get_install_command(provider);
                    el.child(
                        div()
                            .px(px(16.0))
                            .pb(px(12.0))
                            .pl(px(60.0)) // Indent to align with content
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.warning)
                                    .child(format!("⚠️ {} CLI not found", cli_name)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .font_family("monospace")
                                    .child(format!("Install: {}", install_cmd)),
                            ),
                    )
                },
            )
            // API Key configuration (only for API key providers when enabled)
            .when(is_enabled && data.needs_api_key, |el| {
                let has_key = data.has_api_key;
                let api_key_name = data.api_key_name.to_string();
                let provider_name = data.name.clone();
                let accent_color = theme.link;
                let surface_color = theme.selected;
                let success_color = hsla(120.0 / 360.0, 0.6, 0.4, 1.0);
                let muted_color = theme.text_muted;

                el.child(
                    div()
                        .px(px(16.0))
                        .pb(px(12.0))
                        .pl(px(44.0)) // Indent to align with name
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted_color)
                                .min_w(px(60.0))
                                .child("API Key:"),
                        )
                        .child(if has_key {
                            // Key exists - show masked with Clear button
                            let key_name_clear = api_key_name.clone();
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(success_color)
                                        .child("••••••••••••"),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("clear-key-{:?}", provider)))
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(surface_color)
                                        .text_xs()
                                        .text_color(muted_color)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(hover_bg))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |_this, _, _window, cx| {
                                                let _ = exactobar_store::delete_api_key(
                                                    &key_name_clear,
                                                );
                                                cx.notify();
                                            }),
                                        )
                                        .child("Clear"),
                                )
                        } else {
                            // No key - show Configure button
                            let key_name_config = api_key_name.clone();
                            let name_for_dialog = provider_name.clone();
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted_color)
                                        .child("Not configured"),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "config-key-{:?}",
                                            provider
                                        )))
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(accent_color)
                                        .text_xs()
                                        .text_color(white())
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.9))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |_this, _, _window, cx| {
                                                let name = name_for_dialog.clone();
                                                let key_name = key_name_config.clone();
                                                cx.spawn(async move |_, mut cx| {
                                                    if let Some(key) =
                                                        prompt_for_api_key_async(&name).await
                                                    {
                                                        let _ = exactobar_store::store_api_key(
                                                            &key_name, &key,
                                                        );
                                                        // Trigger global state refresh to re-render UI
                                                        let _ = cx.update_global::<AppState, _>(
                                                            |_state, _cx| {
                                                                // State change triggers re-render
                                                            },
                                                        );
                                                    }
                                                })
                                                .detach();
                                            }),
                                        )
                                        .child("Configure"),
                                )
                        }),
                )
            })
    }

    /// Renders the cookie source selector chips.
    fn render_cookie_source_selector(
        &self,
        provider: ProviderKind,
        current: CookieSource,
        theme: SettingsTheme,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .pl(px(44.0)) // Indent to align with name (icon width + gap)
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child("Cookies:"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.0))
                    .children(COOKIE_SOURCES.iter().map(|source| {
                        let is_selected = current == *source;
                        let source_copy = *source;
                        let selected_bg = theme.selected;
                        let default_bg = theme.bg;
                        let accent = theme.link;
                        let border = theme.border;

                        div()
                            .id(SharedString::from(format!(
                                "cookie-{:?}-{:?}",
                                provider, source
                            )))
                            .text_xs()
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .bg(if is_selected { selected_bg } else { default_bg })
                            .border_1()
                            .border_color(if is_selected { accent } else { border })
                            .child(format!("{}", source))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_this, _, _window, cx| {
                                    cx.update_global::<AppState, _>(|state, cx| {
                                        state.settings.update(cx, |model, _| {
                                            model.set_cookie_source(provider, source_copy);
                                        });
                                    });
                                    cx.notify();
                                }),
                            )
                    })),
            )
    }

    /// Renders the data source mode selector chips.
    fn render_data_source_selector(
        &self,
        provider: ProviderKind,
        current: DataSourceMode,
        theme: SettingsTheme,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .pl(px(44.0)) // Indent to align with name
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child("Data source:"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.0))
                    .children(DATA_SOURCE_MODES.iter().map(|mode| {
                        let is_selected = current == *mode;
                        let mode_copy = *mode;
                        let selected_bg = theme.selected;
                        let default_bg = theme.bg;
                        let accent = theme.link;
                        let border = theme.border;

                        div()
                            .id(SharedString::from(format!(
                                "datasrc-{:?}-{:?}",
                                provider, mode
                            )))
                            .text_xs()
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .bg(if is_selected { selected_bg } else { default_bg })
                            .border_1()
                            .border_color(if is_selected { accent } else { border })
                            .child(format!("{}", mode))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_this, _, _window, cx| {
                                    cx.update_global::<AppState, _>(|state, cx| {
                                        state.settings.update(cx, |model, _| match provider {
                                            ProviderKind::Codex => {
                                                model.set_codex_data_source(mode_copy)
                                            }
                                            ProviderKind::Claude => {
                                                model.set_claude_data_source(mode_copy)
                                            }
                                            _ => {}
                                        });
                                    });
                                    cx.notify();
                                }),
                            )
                    })),
            )
    }

    /// Creates a sidebar item with a click handler to switch panes.
    fn sidebar_item(
        &self,
        pane: SettingsPane,
        label: &'static str,
        icon: &'static str,
        is_selected: bool,
        theme: &SettingsTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected_bg = theme.selected;
        let hover_bg = theme.hover;

        let item = div()
            .id(SharedString::from(format!("sidebar-{:?}", pane)))
            .px(px(12.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(8.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.active_pane = pane;
                    cx.notify();
                }),
            )
            .child(div().text_base().child(icon))
            .child(
                div()
                    .text_sm()
                    .when(is_selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                    .child(label),
            );

        if is_selected {
            item.bg(selected_bg)
        } else {
            item.hover(move |s| s.bg(hover_bg))
        }
    }
}
