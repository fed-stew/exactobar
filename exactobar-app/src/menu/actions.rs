//! Action button components for quick provider actions.
//!
//! Provides Dashboard, Status, and Buy Credits buttons that open
//! external URLs based on provider metadata.

use exactobar_core::ProviderKind;
use exactobar_providers::ProviderRegistry;
use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::theme;

// ============================================================================
// URL Opening Helper
// ============================================================================

/// Opens a URL in the default browser.
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let _ = Command::new("open").arg(url).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let _ = Command::new("xdg-open").arg(url).spawn();
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let _ = Command::new("cmd").args(["/c", "start", url]).spawn();
    }
}

// ============================================================================
// Action Buttons Section (Dashboard, Status, Buy Credits)
// ============================================================================

pub struct ActionButtonsSection {
    dashboard_url: Option<String>,
    status_url: Option<String>,
    supports_credits: bool,
    subscription_url: Option<String>,
}

impl ActionButtonsSection {
    pub fn new(provider: ProviderKind) -> Self {
        let descriptor = ProviderRegistry::get(provider);
        let metadata = descriptor.map(|d| &d.metadata);

        Self {
            dashboard_url: metadata.and_then(|m| m.dashboard_url.clone()),
            status_url: metadata.and_then(|m| m.status_link_url.clone()),
            supports_credits: metadata.is_some_and(|m| m.supports_credits),
            subscription_url: metadata.and_then(|m| m.subscription_dashboard_url.clone()),
        }
    }

    /// Returns true if there's at least one button to show.
    fn has_buttons(&self) -> bool {
        self.dashboard_url.is_some()
            || self.status_url.is_some()
            || (self.supports_credits && self.subscription_url.is_some())
    }
}

impl IntoElement for ActionButtonsSection {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        if !self.has_buttons() {
            return div();
        }

        let mut row = div()
            .px(px(14.))
            .py(px(8.))
            .bg(theme::card_background())
            .border_b_1()
            .border_color(theme::glass_separator())
            .flex()
            .gap(px(6.));

        // Dashboard button
        if let Some(url) = self.dashboard_url.clone() {
            row = row.child(ActionButton::new("Dashboard", "⌘D", move || {
                open_url(&url);
            }));
        }

        // Status button
        if let Some(url) = self.status_url.clone() {
            row = row.child(ActionButton::new("Status", "", move || {
                open_url(&url);
            }));
        }

        // Buy Credits button (only if provider supports credits)
        if self.supports_credits {
            if let Some(url) = self.subscription_url.clone() {
                row = row.child(ActionButton::new("Buy Credits...", "", move || {
                    open_url(&url);
                }));
            }
        }

        row
    }
}

// ============================================================================
// Action Button Component
// ============================================================================

struct ActionButton {
    label: &'static str,
    shortcut: &'static str,
    action: Box<dyn Fn() + 'static>,
}

impl ActionButton {
    fn new<F: Fn() + 'static>(label: &'static str, shortcut: &'static str, action: F) -> Self {
        Self {
            label,
            shortcut,
            action: Box::new(action),
        }
    }
}

impl IntoElement for ActionButton {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        let action = self.action;
        let label = self.label;
        let shortcut = self.shortcut;

        let mut btn = div()
            .id(SharedString::from(format!("action-{}", label)))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(4.))
            .cursor_pointer()
            .text_xs()
            .text_color(theme::text_secondary())
            .bg(theme::surface())
            .border_1()
            .border_color(theme::border())
            .hover(|s| s.bg(theme::hover()))
            .active(|s| s.bg(theme::active()))
            .on_mouse_down(MouseButton::Left, move |_, _window, _cx| {
                (action)();
            })
            .flex()
            .items_center()
            .gap(px(4.))
            .child(label);

        if !shortcut.is_empty() {
            btn = btn.child(div().text_xs().text_color(theme::muted()).child(shortcut));
        }

        btn
    }
}
