//! Rich popup menu matching native macOS panel styling.
//!
//! This module provides the main popup menu shown when clicking the tray icon,
//! featuring provider switcher, rich menu cards with progress bars, and working action buttons.
//!
//! Uses transparent backgrounds to let the window's blur effect show through.

#![allow(dead_code)]

use chrono::{DateTime, Local, Utc};
use exactobar_core::{ProviderKind, UsageSnapshot};
use exactobar_providers::ProviderRegistry;
use gpui::prelude::FluentBuilder;
use gpui::*;
use tracing::{debug, info};

use crate::components::{ProviderIcon, Spinner};
use crate::state::AppState;
use crate::theme;
use crate::windows;

// ============================================================================
// Menu Panel
// ============================================================================

/// The main popup panel (replaces TrayMenu).
pub struct MenuPanel {
    /// Currently selected provider (for switcher).
    selected_provider: Option<ProviderKind>,
}

impl MenuPanel {
    /// Creates a new menu panel.
    pub fn new(initial_provider: Option<ProviderKind>) -> Self {
        Self {
            selected_provider: initial_provider,
        }
    }

    /// Renders the provider switcher with WORKING click handlers.
    /// This must be called from render() where we have access to cx.listener().
    fn render_provider_switcher(
        &self,
        providers: &[ProviderKind],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(8.))
            // TRUE LIQUID GLASS: NO background - let window blur shine through!
            .flex()
            .flex_wrap()
            .gap(px(4.))
            .children(providers.iter().map(|&provider| {
                let is_selected = self.selected_provider == Some(provider);
                let name = provider.display_name();

                let mut btn = div()
                    .id(SharedString::from(format!("switch-{:?}", provider)))
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_color(theme::text_primary())
                    // THE MAGIC: cx.listener() gives us access to `this`!
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            info!(provider = ?provider, "Provider switch button clicked!");
                            this.selected_provider = Some(provider);

                            // Check if this provider has data, if not trigger refresh
                            let state = cx.global::<AppState>();
                            let has_snapshot = state.get_snapshot(provider, cx).is_some();
                            if !has_snapshot {
                                info!(provider = ?provider, "No snapshot, triggering refresh");
                                cx.update_global::<AppState, _>(|state, cx| {
                                    state.refresh_provider(provider, cx);
                                });
                            }

                            cx.notify(); // Re-render with new selection!
                        }),
                    );

                if is_selected {
                    btn = btn.bg(theme::accent()).text_color(gpui::white());
                } else {
                    btn = btn
                        .hover(|s| s.bg(theme::hover()))
                        .active(|s| s.bg(theme::active()));
                }

                btn.child(div().text_sm().child(name))
            }))
    }
}

impl Render for MenuPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        info!(provider = ?self.selected_provider, "🎨 MenuPanel::render() called!");

        let state = cx.global::<AppState>();
        let settings = state.settings.read(cx);
        let enabled = state.enabled_providers(cx);
        debug!(enabled_count = enabled.len(), merge_icons = settings.merge_icons(), "Menu state");

        // Determine which provider to show
        let show_provider = self.selected_provider.or_else(|| enabled.first().copied());

        // Build provider card data if we have a provider to show
        let card_data = show_provider.map(|p| MenuCardData::new(p, cx));

        div()
            .id("menu-panel")
            .w(px(340.))  // Slightly wider like Notification Center
            // TRUE LIQUID GLASS: NO background at all! Window blur does everything.
            // NO BORDERS - true borderless liquid glass design
            .rounded(px(14.))  // Smooth rounded corners
            .overflow_hidden()
            // Deep shadow for floating glass effect
            .shadow_lg()
            // Header
            .child(MenuHeader::new())
            // Provider switcher if multiple providers enabled - rendered here for cx.listener() access!
            .when(enabled.len() > 1, |el| {
                el.child(self.render_provider_switcher(&enabled, cx))
            })
            // Menu card for selected provider
            .when_some(card_data, |el, data| el.child(MenuCard::new(data)))
            // Action footer with WORKING buttons
            .child(MenuFooter::new())
    }
}

// ============================================================================
// Menu Header
// ============================================================================

struct MenuHeader;

impl MenuHeader {
    fn new() -> Self {
        Self
    }
}

impl IntoElement for MenuHeader {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        div()
            .px(px(14.))
            .py(px(10.))
            // TRUE LIQUID GLASS: NO background - let window blur shine through!
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text_primary())
                            .child("ExactoBar"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::muted())
                            .child(env!("CARGO_PKG_VERSION")),
                    ),
            )
    }
}

// ============================================================================
// Provider Switcher - REMOVED!
// ============================================================================
// NOTE: The ProviderSwitcher and ProviderSwitchButton structs were removed because
// IntoElement::into_element() doesn't have access to cx.listener(), so we couldn't
// add click handlers. The rendering logic is now in MenuPanel::render_provider_switcher()
// where we have proper access to the context.

// ============================================================================
// Menu Card Data
// ============================================================================

struct MenuCardData {
    provider: ProviderKind,
    provider_name: String,
    email: String,
    plan: Option<String>,
    snapshot: Option<UsageSnapshot>,
    is_refreshing: bool,
    error: Option<String>,
    session_label: &'static str,
    weekly_label: &'static str,
    /// Whether to show "X% used" instead of "X% remaining"
    show_used: bool,
    /// Whether to show "Resets at 3:00 PM" instead of "Resets in 2h 30m"
    show_absolute: bool,
}

impl MenuCardData {
    fn new<V: 'static>(provider: ProviderKind, cx: &Context<V>) -> Self {
        let state = cx.global::<AppState>();
        let snapshot = state.get_snapshot(provider, cx);
        let is_refreshing = state.is_provider_refreshing(provider, cx);
        let error = state.get_error(provider, cx);
        let descriptor = ProviderRegistry::get(provider);

        // Read display settings
        let settings = state.settings.read(cx).settings();
        let show_used = settings.usage_bars_show_used;
        let show_absolute = settings.reset_times_show_absolute;

        let provider_name = descriptor
            .map(|d| d.display_name().to_string())
            .unwrap_or_else(|| format!("{:?}", provider));

        let session_label = descriptor
            .map(|d| d.metadata.session_label.as_str())
            .unwrap_or("Session");

        let weekly_label = descriptor
            .map(|d| d.metadata.weekly_label.as_str())
            .unwrap_or("Weekly");

        // Extract identity info from snapshot
        let identity = snapshot.as_ref().and_then(|s| s.identity.as_ref());
        let email = identity
            .and_then(|i| i.account_email.as_deref())
            .unwrap_or("")
            .to_string();
        let plan = identity.and_then(|i| i.plan_name.clone());

        Self {
            provider,
            provider_name,
            email,
            plan,
            snapshot,
            is_refreshing,
            error,
            session_label,
            weekly_label,
            show_used,
            show_absolute,
        }
    }
}

// ============================================================================
// Menu Card
// ============================================================================

struct MenuCard {
    data: MenuCardData,
}

impl MenuCard {
    fn new(data: MenuCardData) -> Self {
        Self { data }
    }
}

impl IntoElement for MenuCard {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let mut card = div().flex().flex_col();

        // Header section
        card = card.child(CardHeader {
            provider: self.data.provider,
            provider_name: self.data.provider_name.clone(),
            email: self.data.email.clone(),
            plan: self.data.plan.clone(),
            is_refreshing: self.data.is_refreshing,
            has_error: self.data.error.is_some(),
        });

        // Error display
        if let Some(ref err) = self.data.error {
            card = card.child(ErrorSection {
                message: err.clone(),
            });
        } else if let Some(ref snap) = self.data.snapshot {
            // Usage metrics
            card = card.child(UsageMetricsSection::new(
                snap,
                self.data.session_label,
                self.data.weekly_label,
                self.data.show_used,
                self.data.show_absolute,
            ));
        } else if !self.data.is_refreshing {
            card = card.child(PlaceholderSection);
        }

        card
    }
}

// ============================================================================
// Card Header
// ============================================================================

struct CardHeader {
    provider: ProviderKind,
    provider_name: String,
    email: String,
    plan: Option<String>,
    is_refreshing: bool,
    has_error: bool,
}

impl IntoElement for CardHeader {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let status_text = if self.is_refreshing {
            "Refreshing...".to_string()
        } else if self.has_error {
            "Error".to_string()
        } else {
            "Updated just now".to_string()
        };

        let status_color = if self.has_error {
            theme::error()
        } else {
            theme::muted()
        };

        // Build top row with optional email
        let mut top_row = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ProviderIcon::new(self.provider).size(px(18.)))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text_primary())
                            .child(self.provider_name),
                    ),
            );

        if !self.email.is_empty() {
            top_row = top_row.child(
                div()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(self.email),
            );
        }

        // Build status row with optional spinner
        let mut status_row = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .text_xs()
                    .text_color(status_color)
                    .child(status_text),
            );

        if self.is_refreshing {
            status_row = status_row.child(Spinner::new());
        }

        // Build bottom row with optional plan
        let mut bottom_row = div()
            .flex()
            .items_center()
            .justify_between()
            .child(status_row);

        if let Some(plan) = self.plan {
            bottom_row = bottom_row.child(
                div()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(plan),
            );
        }

        div()
            .px(px(14.))
            .py(px(10.))
            // TRUE LIQUID GLASS: NO background - let window blur shine through!
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(top_row)
            .child(bottom_row)
    }
}

// ============================================================================
// Usage Metrics Section
// ============================================================================

struct UsageMetricsSection {
    metrics: Vec<UsageMetric>,
}

struct UsageMetric {
    title: String,
    used_percent: f64,
    resets_at: Option<DateTime<Utc>>,
    reset_description: Option<String>,
    /// When true, show "X% used" instead of "X% remaining"
    show_used: bool,
    /// When true, show "Resets at 3:00 PM" instead of "Resets in 2h 30m"
    show_absolute: bool,
}

impl UsageMetricsSection {
    fn new(
        snapshot: &UsageSnapshot,
        session_label: &str,
        weekly_label: &str,
        show_used: bool,
        show_absolute: bool,
    ) -> Self {
        let mut metrics = Vec::new();

        if let Some(primary) = &snapshot.primary {
            metrics.push(UsageMetric {
                title: session_label.to_string(),
                used_percent: primary.used_percent,
                resets_at: primary.resets_at,
                reset_description: primary.reset_description.clone(),
                show_used,
                show_absolute,
            });
        }

        if let Some(secondary) = &snapshot.secondary {
            metrics.push(UsageMetric {
                title: weekly_label.to_string(),
                used_percent: secondary.used_percent,
                resets_at: secondary.resets_at,
                reset_description: secondary.reset_description.clone(),
                show_used,
                show_absolute,
            });
        }

        if let Some(tertiary) = &snapshot.tertiary {
            metrics.push(UsageMetric {
                title: "Premium".to_string(),
                used_percent: tertiary.used_percent,
                resets_at: tertiary.resets_at,
                reset_description: tertiary.reset_description.clone(),
                show_used,
                show_absolute,
            });
        }

        Self { metrics }
    }
}

impl IntoElement for UsageMetricsSection {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        if self.metrics.is_empty() {
            return div();
        }

        div()
            .px(px(14.))
            .py(px(10.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .children(self.metrics.into_iter().map(UsageMetricRow::new))
    }
}

struct UsageMetricRow {
    metric: UsageMetric,
}

impl UsageMetricRow {
    fn new(metric: UsageMetric) -> Self {
        Self { metric }
    }

    /// Format reset time based on settings.
    /// Returns "Resets at 3:00 PM" or "Resets in 2h 30m" depending on `show_absolute`.
    fn format_reset_time(&self) -> Option<String> {
        if self.metric.show_absolute {
            // Absolute time format: "Resets at 3:00 PM"
            self.metric.resets_at.map(|reset_at| {
                let local_time: DateTime<Local> = reset_at.into();
                format!("Resets at {}", local_time.format("%l:%M %p").to_string().trim())
            })
        } else {
            // Relative time format: "Resets in 2h 30m" or use provider's description
            if let Some(reset_at) = self.metric.resets_at {
                let now = Utc::now();
                if reset_at > now {
                    let duration = reset_at - now;
                    let total_minutes = duration.num_minutes();
                    let hours = total_minutes / 60;
                    let minutes = total_minutes % 60;

                    let time_str = if hours > 0 {
                        format!("{}h {}m", hours, minutes)
                    } else {
                        format!("{}m", minutes)
                    };
                    Some(format!("Resets in {}", time_str))
                } else {
                    Some("Resets soon".to_string())
                }
            } else {
                // Fall back to provider's description if no timestamp
                self.metric.reset_description.as_ref().map(|d| format!("Resets {}", d))
            }
        }
    }
}

impl IntoElement for UsageMetricRow {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let used_percent = self.metric.used_percent.clamp(0.0, 100.0);
        let remaining_percent = 100.0 - used_percent;

        // Determine label based on settings
        let percent_label = if self.metric.show_used {
            format!("{:.0}% used", used_percent)
        } else {
            format!("{:.0}% remaining", remaining_percent)
        };

        // Color is based on remaining percent (low remaining = warning/error)
        let color = theme::color_for_percent(remaining_percent);

        // Progress bar fill: when showing "used", fill represents used amount
        // when showing "remaining", fill represents remaining amount
        let bar_fill_percent = if self.metric.show_used {
            used_percent
        } else {
            remaining_percent
        };

        // Format reset time based on settings
        let reset_text = self.format_reset_time();

        // Build footer row with optional reset text
        let mut footer_row = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_secondary())
                    .child(percent_label),
            );

        if let Some(text) = reset_text {
            footer_row = footer_row.child(
                div()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(text),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            // Title
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::text_primary())
                    .child(self.metric.title),
            )
            // Capsule-shaped progress bar
            .child(ProgressBar::new(bar_fill_percent, color))
            // Footer
            .child(footer_row)
    }
}

// ============================================================================
// Progress Bar (Capsule Style like CodexBar)
// ============================================================================

struct ProgressBar {
    percent: f64,
    color: Hsla,
}

impl ProgressBar {
    fn new(percent: f64, color: Hsla) -> Self {
        Self {
            percent: percent.clamp(0.0, 100.0),
            color,
        }
    }
}

impl IntoElement for ProgressBar {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let fraction = (self.percent / 100.0) as f32;

        // Capsule-shaped progress bar: 6px height, fully rounded ends (radius = height/2)
        div()
            .h(px(6.))
            .w_full()
            .bg(theme::track())
            .rounded(px(3.)) // Full capsule shape
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .w(relative(fraction))
                    .bg(self.color)
                    .rounded(px(3.)), // Match container rounding
            )
    }
}

// ============================================================================
// Error & Placeholder Sections
// ============================================================================

struct ErrorSection {
    message: String,
}

impl IntoElement for ErrorSection {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        div().px(px(14.)).py(px(10.)).child(
            div()
                .text_sm()
                .text_color(theme::error())
                .child(self.message),
        )
    }
}

struct PlaceholderSection;

impl IntoElement for PlaceholderSection {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        div().px(px(14.)).py(px(10.)).child(
            div()
                .text_sm()
                .text_color(theme::muted())
                .child("No data yet"),
        )
    }
}

// ============================================================================
// Menu Footer with WORKING Buttons
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
            .child(
                div()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(shortcut),
            )
    }
}

// ============================================================================
// Legacy TrayMenu Alias
// ============================================================================

/// Alias for backwards compatibility.
pub type TrayMenu = MenuPanel;
