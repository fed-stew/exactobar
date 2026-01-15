// Lint configuration for this crate
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

//! ExactoBar - GPUI Menu Bar Application
//!
//! A macOS menu bar app for monitoring LLM provider usage.

mod actions;
mod components;
mod icon;
mod menu;
mod notifications;
mod refresh;
mod state;
mod theme;
mod tray;
mod windows;

use gpui::*;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::state::AppState;
use crate::tray::SystemTray;

/// Application entry point.
fn main() {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    info!("ExactoBar starting...");

    // Run the GPUI application
    Application::new().run(|cx: &mut App| {
        // Register actions
        actions::register_actions(cx);

        // Initialize global state
        let state = AppState::init(cx);
        cx.set_global(state);

        // Initialize system tray
        let tray = SystemTray::new(cx);
        cx.set_global(tray);

        // Start the click listener for status item interactions
        cx.update_global::<SystemTray, _>(|tray, cx| {
            tray.start_click_listener(cx);
        });

        // Debug: write icon PNG to temp file for verification
        #[cfg(debug_assertions)]
        {
            let tray = cx.global::<SystemTray>();
            let state = cx.global::<AppState>();
            let providers = state.enabled_providers(cx);
            if let Some(provider) = providers.first() {
                if let Some(png) = tray.get_icon_png(*provider, cx) {
                    if std::fs::write("/tmp/exactobar-icon.png", &png).is_ok() {
                        info!(provider = ?provider, "Wrote debug icon to /tmp/exactobar-icon.png ({} bytes)", png.len());
                    }
                }
            }
        }

        // Start background refresh task
        refresh::spawn_refresh_task(cx);

        // Check for onboarding - open settings if no providers
        if should_show_onboarding(cx) {
            windows::open_settings(cx);
        }

        info!("ExactoBar initialized");
    });
}

/// Checks if we should show onboarding (first run or no providers).
fn should_show_onboarding(cx: &App) -> bool {
    let state = cx.global::<AppState>();
    state.enabled_providers(cx).is_empty()
}
