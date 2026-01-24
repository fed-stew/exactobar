//! General settings pane.

use exactobar_store::{RefreshCadence, ThemeMode};
use gpui::prelude::*;
use gpui::*;

use super::SettingsTheme;
use crate::components::Toggle;
use crate::state::AppState;

/// General settings pane.
pub struct GeneralPane {
    cadence: RefreshCadence,
    merge_icons: bool,
    theme_mode: ThemeMode,
    usage_bars_show_used: bool,
    reset_times_show_absolute: bool,
    menu_bar_shows_brand_icon_with_percent: bool,
    switcher_shows_icons: bool,
    theme: SettingsTheme,
}

impl GeneralPane {
    pub fn new<V: 'static>(cx: &Context<V>, theme: SettingsTheme) -> Self {
        let state = cx.global::<AppState>();
        let settings = state.settings.read(cx).settings();
        Self {
            cadence: settings.refresh_cadence,
            merge_icons: settings.merge_icons,
            theme_mode: settings.theme_mode,
            usage_bars_show_used: settings.usage_bars_show_used,
            reset_times_show_absolute: settings.reset_times_show_absolute,
            menu_bar_shows_brand_icon_with_percent: settings.menu_bar_shows_brand_icon_with_percent,
            switcher_shows_icons: settings.switcher_shows_icons,
            theme,
        }
    }
}

impl IntoElement for GeneralPane {
    type Element = Div;

    fn into_element(self) -> Self::Element {
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
                            .child("General"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child("Configure ExactoBar behavior"),
                    ),
            )
            .child(render_cadence_section(self.cadence, theme))
            .child(render_icon_section(self.merge_icons, theme))
            .child(render_theme_section(self.theme_mode, theme))
            .child(render_display_section(
                self.usage_bars_show_used,
                self.reset_times_show_absolute,
                self.menu_bar_shows_brand_icon_with_percent,
                self.switcher_shows_icons,
                theme,
            ))
    }
}

fn render_cadence_section(current: RefreshCadence, theme: SettingsTheme) -> Div {
    let options = [
        (RefreshCadence::Manual, "Manual"),
        (RefreshCadence::OneMinute, "Every minute"),
        (RefreshCadence::TwoMinutes, "Every 2 minutes"),
        (RefreshCadence::FiveMinutes, "Every 5 minutes"),
        (RefreshCadence::FifteenMinutes, "Every 15 minutes"),
    ];

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Refresh Cadence"),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("How often to automatically refresh usage data"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .children(options.iter().map(|(cadence, label)| {
                    render_radio_option(*cadence, label, current == *cadence, theme)
                })),
        )
}

fn render_radio_option(
    cadence: RefreshCadence,
    label: &'static str,
    selected: bool,
    theme: SettingsTheme,
) -> Div {
    let hover_bg = theme.hover;
    div()
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(px(12.0))
        .when(selected, |el| el.bg(theme.selected))
        .when(!selected, |el| el.hover(move |s| s.bg(hover_bg)))
        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
            cx.update_global::<AppState, _>(|state, cx| {
                state.settings.update(cx, |model, _| {
                    model.set_refresh_cadence(cadence);
                });
            });
        })
        .child(
            div()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_full()
                .border_2()
                .border_color(if selected { theme.link } else { theme.border })
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |el| {
                    el.child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(theme.link))
                }),
        )
        .child(div().text_sm().child(label))
}

fn render_icon_section(merge_icons: bool, theme: SettingsTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Menu Bar Icons"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .py(px(8.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(div().text_sm().child("Merge icons"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Show a single icon instead of one per provider"),
                        ),
                )
                .child(
                    Toggle::new("toggle-merge-icons")
                        .checked(merge_icons)
                        .on_toggle(|enabled, cx| {
                            cx.update_global::<AppState, _>(|state, cx| {
                                state.settings.update(cx, |model, _| {
                                    model.set_merge_icons(enabled);
                                });
                            });
                        }),
                ),
        )
}

fn render_theme_section(current: ThemeMode, theme: SettingsTheme) -> Div {
    let options: Vec<(ThemeMode, &'static str, &'static str)> = vec![
        (
            ThemeMode::Dark,
            "Dark",
            "Always use dark theme (recommended for liquid glass effect)",
        ),
        (
            ThemeMode::Light,
            "Light",
            "Use light theme for better readability on bright backgrounds",
        ),
        (
            ThemeMode::System,
            "System",
            "Follow system appearance (auto-switch based on OS setting)",
        ),
    ];

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Theme"),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("Choose your preferred appearance"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .children(options.into_iter().map(move |(mode, label, description)| {
                    let is_selected = current == mode;
                    let hover_bg = theme.hover;
                    div()
                        .px(px(12.0))
                        .py(px(8.0))
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .when(is_selected, |el| el.bg(theme.selected))
                        .when(!is_selected, |el| el.hover(move |s| s.bg(hover_bg)))
                        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                            let mode = mode;
                            // Get settings entity first, then update (avoids nested borrows)
                            let settings = cx.global::<AppState>().settings.clone();
                            settings.update(cx, |model, cx| {
                                model.set_theme_mode(mode);
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .rounded_full()
                                .border_2()
                                .border_color(if is_selected {
                                    theme.link
                                } else {
                                    theme.border
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(is_selected, |el| {
                                    el.child(
                                        div().w(px(8.0)).h(px(8.0)).rounded_full().bg(theme.link),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(label))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child(description),
                                ),
                        )
                })),
        )
}

fn render_display_section(
    usage_bars_show_used: bool,
    reset_times_show_absolute: bool,
    menu_bar_shows_brand_icon_with_percent: bool,
    switcher_shows_icons: bool,
    theme: SettingsTheme,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Display Options"),
        )
        // Show used percent toggle
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
                                .child("Show Used Percent"),
                        )
                        .child(
                            div().text_xs().text_color(theme.text_muted).child(
                                "Progress bars show percent used instead of percent remaining",
                            ),
                        ),
                )
                .child(
                    Toggle::new("toggle-usage-bars-show-used")
                        .checked(usage_bars_show_used)
                        .on_toggle(|enabled, cx| {
                            cx.update_global::<AppState, _>(|state, cx| {
                                state.settings.update(cx, |model, _| {
                                    model.set_usage_bars_show_used(enabled);
                                });
                            });
                        }),
                ),
        )
        // Absolute reset times toggle
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
                                .child("Absolute Reset Times"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Show reset times as clock values instead of countdowns"),
                        ),
                )
                .child(
                    Toggle::new("toggle-reset-times-absolute")
                        .checked(reset_times_show_absolute)
                        .on_toggle(|enabled, cx| {
                            cx.update_global::<AppState, _>(|state, cx| {
                                state.settings.update(cx, |model, _| {
                                    model.set_reset_times_show_absolute(enabled);
                                });
                            });
                        }),
                ),
        )
        // Brand icon with percent toggle
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
                                .child("Brand Icon with Percent"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Show provider icon and percentage in menu bar"),
                        ),
                )
                .child(
                    Toggle::new("toggle-menu-bar-brand-icon-percent")
                        .checked(menu_bar_shows_brand_icon_with_percent)
                        .on_toggle(|enabled, cx| {
                            cx.update_global::<AppState, _>(|state, cx| {
                                state.settings.update(cx, |model, _| {
                                    model.set_menu_bar_shows_brand_icon_with_percent(enabled);
                                });
                            });
                        }),
                ),
        )
        // Switcher shows icons toggle
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .py(px(12.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child("Switcher Shows Icons"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Show provider icons in the quick switcher"),
                        ),
                )
                .child(
                    Toggle::new("toggle-switcher-shows-icons")
                        .checked(switcher_shows_icons)
                        .on_toggle(|enabled, cx| {
                            cx.update_global::<AppState, _>(|state, cx| {
                                state.settings.update(cx, |model, _| {
                                    model.set_switcher_shows_icons(enabled);
                                });
                            });
                        }),
                ),
        )
}
