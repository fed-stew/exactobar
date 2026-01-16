//! Menu footer with action buttons (Refresh, Settings, Quit).
//!
//! These buttons actually work - they trigger real actions through
//! the global AppState and window management.

use gpui::*;
use tracing::info;

use crate::state::AppState;
use crate::theme;
use crate::windows;

// ============================================================================
// Menu Footer
// ============================================================================

pub struct MenuFooter;

impl MenuFooter {
    pub fn new() -> Self {
        Self
    }
}

impl IntoElement for MenuFooter {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        tracing::trace!("MenuFooter rendering footer buttons");
        div()
            .px(px(10.))
            .py(px(8.))
            // TRUE LIQUID GLASS: NO background - let window blur shine through!
            .flex()
            .items_center()
            .justify_between()
            // Refresh button - ACTUALLY REFRESHES
            .child(FooterActionButton::refresh())
            // Settings button - OPENS SETTINGS
            .child(FooterActionButton::settings())
            // Quit button - ACTUALLY QUITS
            .child(FooterActionButton::quit())
    }
}

impl Default for MenuFooter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Footer Action Buttons (Interactive!)
// ============================================================================

/// Action types for footer buttons.
#[derive(Clone, Copy, Debug)]
enum FooterAction {
    Refresh,
    Settings,
    Quit,
}

/// A footer button that actually does something!
struct FooterActionButton {
    action: FooterAction,
    label: &'static str,
    shortcut: &'static str,
}

impl FooterActionButton {
    fn refresh() -> Self {
        Self {
            action: FooterAction::Refresh,
            label: "Refresh",
            shortcut: "⌘R",
        }
    }

    fn settings() -> Self {
        Self {
            action: FooterAction::Settings,
            label: "Settings...",
            shortcut: "⌘,",
        }
    }

    fn quit() -> Self {
        Self {
            action: FooterAction::Quit,
            label: "Quit",
            shortcut: "⌘Q",
        }
    }
}

impl IntoElement for FooterActionButton {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        let action = self.action;
        let label = self.label;
        let shortcut = self.shortcut;

        tracing::trace!(button = label, "FooterActionButton rendering");

        div()
            .id(SharedString::from(label))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(6.))
            .cursor_pointer()
            // Hover state - subtle highlight like native macOS
            .hover(|s| s.bg(theme::hover()))
            // Active/pressed state - stronger highlight
            .active(|s| s.bg(theme::active()))
            // Click handler - THE MAGIC HAPPENS HERE!
            .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                info!(action = ?action, "Footer button clicked!");
                match action {
                    FooterAction::Refresh => {
                        // Trigger refresh of all providers
                        cx.update_global::<AppState, _>(|state, cx| {
                            state.refresh_all(cx);
                        });
                    }
                    FooterAction::Settings => {
                        tracing::trace!("Settings button clicked, opening settings window");
                        let task = cx.spawn(async move |mut cx| {
                            cx.update(|cx| {
                                windows::open_settings(cx);
                            });
                        });
                        task.detach();
                    }
                    FooterAction::Quit => {
                        // Quit the application
                        cx.quit();
                    }
                }
            })
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                div()
                    .text_sm()
                    .text_color(theme::text_primary())
                    .child(label),
            )
            .child(div().text_xs().text_color(theme::muted()).child(shortcut))
    }
}
