//! Background refresh task.
//!
//! Handles periodic refreshing of provider usage data.
//! Uses a dedicated Tokio runtime for fetch operations since the
//! fetch/providers libraries are Tokio-based while GPUI uses smol.

#![allow(dead_code)]

use std::sync::OnceLock;
use std::time::Duration;

use exactobar_core::{ProviderKind, UsageSnapshot};
use exactobar_fetch::FetchContext;
use exactobar_providers::ProviderRegistry;
use gpui::*;
use smol::Timer;
use tracing::{debug, error, info};

use crate::notifications::{send_quota_notification, NotificationTracker};
use crate::state::{AppState, UsageModel};

/// Global notification tracker for quota alerts.
/// Uses Lazy<Mutex<>> to avoid spamming notifications across refresh cycles.
static NOTIFICATION_TRACKER: once_cell::sync::Lazy<std::sync::Mutex<NotificationTracker>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(NotificationTracker::new()));

/// Global Tokio runtime for fetch operations.
/// We need this because the fetch/providers libraries use tokio::process::Command
/// which requires a Tokio runtime, but GPUI runs on smol.
static TOKIO_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Gets or creates the Tokio runtime for fetch operations.
///
/// # Panics
///
/// Panics if the Tokio runtime cannot be created. This is intentionally
/// unrecoverable because:
/// 1. The fetch/providers libraries require a Tokio runtime (they use
///    `tokio::process::Command` which panics without one)
/// 2. Runtime creation only fails due to OS resource exhaustion or
///    misconfiguration, making recovery impossible
/// 3. The application cannot function without fetching provider data
///
/// This uses `OnceLock` so the panic can only occur once at initialization.
fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect(
                "Failed to create Tokio runtime for fetch operations. \
                This is unrecoverable - the application cannot fetch provider data."
            )
    })
}

/// Spawns the background refresh task.
pub fn spawn_refresh_task(cx: &mut App) {
    info!("Starting background refresh task");

    // Get initial data before spawning
    let state = cx.global::<AppState>();
    let initial_providers = state.enabled_providers(cx);
    let usage = state.usage.clone();

    cx.spawn(async move |mut cx| {
        // Initial refresh after a short delay
        Timer::after(Duration::from_secs(2)).await;

        for provider in &initial_providers {
            refresh_provider(*provider, usage.clone(), &mut cx).await;
        }

        loop {
            // Get refresh cadence from settings - try to get duration, default to 5 minutes
            let duration_result = cx.update(|cx| {
                let state = cx.global::<AppState>();
                state.settings.read(cx).refresh_cadence().as_duration()
            });

            let duration: Duration = match duration_result {
                Some(d) => d,
                None => {
                    // Manual mode or error - sleep 60 seconds and loop
                    Timer::after(Duration::from_secs(60)).await;
                    continue;
                }
            };

            debug!("Sleeping {} seconds until next refresh", duration.as_secs());
            Timer::after(duration).await;

            // Get current providers and refresh
            let providers_result = cx.update(|cx| {
                let state = cx.global::<AppState>();
                state.enabled_providers(cx)
            });

            if let Some(providers) = Some(providers_result) {
                for provider in providers {
                    refresh_provider(provider, usage.clone(), &mut cx).await;
                }
            }
        }
    })
    .detach();
}

/// Executes a fetch operation on the Tokio runtime.
/// This bridges the smol-based GPUI world with the tokio-based fetch world.
/// 
/// **IMPORTANT**: All fetch operations MUST go through this function!
/// The fetch/providers libraries use tokio::process::Command which requires
/// a Tokio runtime. Calling them directly from smol will panic.
pub async fn fetch_on_tokio(provider: ProviderKind) -> Result<UsageSnapshot, String> {
    let rt = tokio_runtime();
    
    // Use spawn_blocking to run the tokio future on the tokio runtime
    // from within a smol context
    let result = smol::unblock(move || {
        rt.block_on(async move {
            let ctx = FetchContext::new();
            if let Some(desc) = ProviderRegistry::get(provider) {
                let pipeline = desc.build_pipeline(&ctx);
                let outcome = pipeline.execute(&ctx).await;
                
                match outcome.result {
                    Ok(fetch_result) => {
                        debug!(
                            "Provider {:?} fetch succeeded with strategy {:?}",
                            provider, fetch_result.strategy_id
                        );
                        Ok(fetch_result.snapshot)
                    }
                    Err(e) => {
                        error!("Provider {:?} fetch failed: {}", provider, e);
                        Err(e.to_string())
                    }
                }
            } else {
                Err("Provider not found".to_string())
            }
        })
    })
    .await;
    
    result
}

/// Refreshes a single provider.
async fn refresh_provider(
    provider: ProviderKind,
    usage: Entity<UsageModel>,
    cx: &mut AsyncApp,
) {
    debug!("Refreshing provider {:?}", provider);

    // Mark as refreshing
    let _ = cx.update_entity(&usage, |model, cx| {
        model.set_refreshing(provider, true);
        cx.notify();
    });

    // Execute fetch on Tokio runtime
    let result = fetch_on_tokio(provider).await;

    // Check if notifications are enabled before we move result
    let notify_enabled = cx
        .update(|cx| {
            cx.global::<AppState>()
                .settings
                .read(cx)
                .settings()
                .session_quota_notifications_enabled
        });

    // Check for quota notifications on successful fetch
    if let Ok(ref snapshot) = result {
        if notify_enabled {
            if let Ok(mut tracker) = NOTIFICATION_TRACKER.lock() {
                if let Some(level) = tracker.should_notify(provider, snapshot) {
                    let percent = snapshot
                        .primary
                        .as_ref()
                        .map(|w| w.used_percent)
                        .unwrap_or(0.0);
                    send_quota_notification(provider, level, percent);
                }
            }
        }
    }

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
}

/// Triggers an immediate refresh of all providers.
pub fn trigger_refresh(cx: &mut App) {
    let state = cx.global::<AppState>();
    let providers = state.enabled_providers(cx);
    let usage = state.usage.clone();

    cx.spawn(async move |mut cx| {
        for provider in providers {
            refresh_provider(provider, usage.clone(), &mut cx).await;
        }
    })
    .detach();
}
