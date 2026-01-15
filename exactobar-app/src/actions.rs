//! Application actions.
//!
//! Simple action handlers for the app.

use exactobar_core::ProviderKind;
use gpui::*;
use tracing::info;

use crate::state::{AppState, UsageModel};
use crate::windows;

/// Registers all application actions.
pub fn register_actions(_cx: &mut App) {
    // Actions are handled via callbacks in the UI, not via global action dispatch
    info!("Actions registered");
}

/// Refreshes all enabled providers.
pub fn refresh_all(cx: &mut App) {
    let state = cx.global::<AppState>();
    let providers = state.enabled_providers(cx);
    let usage = state.usage.clone();

    for provider in providers {
        refresh_provider_async(provider, usage.clone(), cx);
    }
}

/// Opens the settings window.
pub fn open_settings(cx: &mut App) {
    windows::open_settings(cx);
}

/// Quits the application.
pub fn quit(cx: &mut App) {
    cx.quit();
}

/// Refreshes a provider asynchronously.
fn refresh_provider_async(
    provider: ProviderKind,
    usage: Entity<UsageModel>,
    cx: &mut App,
) {
    cx.spawn(async move |mut cx| {
        // Mark as refreshing
        let _ = cx.update_entity(&usage, |model, cx| {
            model.set_refreshing(provider, true);
            cx.notify();
        });

        // Execute fetch on Tokio runtime - MUST use this bridge!
        // Direct pipeline.execute() calls will panic because tokio::process::Command
        // requires a Tokio runtime, but GPUI runs on smol.
        let result = crate::refresh::fetch_on_tokio(provider).await;

        // Update state
        let _ = cx.update_entity(&usage, |model, cx| {
            model.set_refreshing(provider, false);
            match result {
                Ok(snapshot) => {
                    model.set_snapshot(provider, snapshot);
                    model.clear_error(provider);
                }
                Err(e) => {
                    model.set_error(provider, e);
                }
            }
            cx.notify();
        });
    })
    .detach();
}
