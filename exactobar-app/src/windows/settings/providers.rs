//! Providers settings pane - helper types and functions.

use exactobar_core::ProviderKind;
use exactobar_providers::ProviderRegistry;
use exactobar_store::{CookieSource, DataSourceMode};
use gpui::Context;

use crate::state::AppState;

/// Provider row data for rendering.
pub struct ProviderRowData {
    pub provider: ProviderKind,
    pub is_enabled: bool,
    pub name: String,
    pub cli_name: String,
    pub is_primary: bool,
    pub supports_cookies: bool,
    pub supports_data_source: bool,
    pub current_cookie_source: CookieSource,
    pub current_data_source: Option<DataSourceMode>,
}

/// Check if a provider supports cookie-based web fetching.
pub fn provider_supports_cookies(provider: ProviderKind) -> bool {
    matches!(
        provider,
        ProviderKind::Codex
            | ProviderKind::Claude
            | ProviderKind::Cursor
            | ProviderKind::Factory
            | ProviderKind::MiniMax
            | ProviderKind::Augment
    )
}

/// Check if a provider supports data source mode selection.
pub fn provider_supports_data_source(provider: ProviderKind) -> bool {
    matches!(provider, ProviderKind::Codex | ProviderKind::Claude)
}

/// Collect all provider data for rendering.
pub fn collect_provider_data<V: 'static>(cx: &Context<V>) -> Vec<ProviderRowData> {
    let state = cx.global::<AppState>();
    let settings = state.settings.read(cx);
    let all_providers = ProviderRegistry::all();

    all_providers.iter().map(|desc| {
        let provider = desc.id;
        let is_enabled = settings.is_provider_enabled(provider);
        let supports_cookies = provider_supports_cookies(provider);
        let supports_data_source = provider_supports_data_source(provider);
        let current_cookie_source = settings.cookie_source(provider);
        let current_data_source = if supports_data_source {
            Some(match provider {
                ProviderKind::Codex => settings.codex_data_source(),
                ProviderKind::Claude => settings.claude_data_source(),
                _ => DataSourceMode::Auto,
            })
        } else {
            None
        };

        ProviderRowData {
            provider,
            is_enabled,
            name: desc.display_name().to_string(),
            cli_name: desc.cli_name().to_string(),
            is_primary: desc.metadata.is_primary_provider,
            supports_cookies,
            supports_data_source,
            current_cookie_source,
            current_data_source,
        }
    }).collect()
}

/// Cookie source options for the selector.
pub const COOKIE_SOURCES: [CookieSource; 6] = [
    CookieSource::Auto,
    CookieSource::Safari,
    CookieSource::Chrome,
    CookieSource::Arc,
    CookieSource::Firefox,
    CookieSource::Off,
];

/// Data source mode options for the selector.
pub const DATA_SOURCE_MODES: [DataSourceMode; 4] = [
    DataSourceMode::Auto,
    DataSourceMode::Cli,
    DataSourceMode::Web,
    DataSourceMode::Api,
];
