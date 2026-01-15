//! Cursor provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};
use std::path::PathBuf;

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::{CursorLocalStrategy, CursorWebStrategy};

/// Creates the Cursor provider descriptor.
pub fn cursor_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Cursor,
        metadata: cursor_metadata(),
        branding: cursor_branding(),
        token_cost: cursor_token_cost(),
        fetch_plan: cursor_fetch_plan(),
        cli: cursor_cli_config(),
    }
}

/// Cursor metadata configuration.
fn cursor_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Cursor,
        display_name: "Cursor".to_string(),
        session_label: "Requests".to_string(),
        weekly_label: "Monthly".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: true,
        credits_hint: "Pro plan credits".to_string(),
        toggle_title: "Show Cursor usage".to_string(),
        cli_name: "cursor".to_string(),
        default_enabled: false, // Not enabled by default
        is_primary_provider: false,
        uses_account_fallback: false,
        dashboard_url: Some("https://cursor.com/settings".to_string()),
        subscription_dashboard_url: Some("https://cursor.com/settings/billing".to_string()),
        status_page_url: None,
        status_link_url: Some("https://status.cursor.com".to_string()),
    }
}

/// Cursor branding configuration.
fn cursor_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Cursor,
        icon_resource_name: "icon_cursor".to_string(),
        color: ProviderColor::new(0.4, 0.4, 0.4), // Cursor gray
    }
}

/// Cursor token cost configuration.
fn cursor_token_cost() -> TokenCostConfig {
    TokenCostConfig {
        supports_token_cost: false, // Cursor uses credits, not tokens
        log_directory: None,
    }
}

/// Cursor fetch plan.
fn cursor_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::Web, SourceMode::Auto],
        build_pipeline: build_cursor_pipeline,
    }
}

/// Builds the Cursor fetch pipeline.
fn build_cursor_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    // Web cookie strategy (primary)
    if ctx.settings.source_mode.allows_web() {
        strategies.push(Box::new(CursorWebStrategy::new()));
    }

    // Local strategy (fallback)
    strategies.push(Box::new(CursorLocalStrategy::new()));

    FetchPipeline::with_strategies(strategies)
}

/// Cursor CLI configuration (limited - Cursor doesn't have a full CLI).
fn cursor_cli_config() -> CliConfig {
    CliConfig {
        name: "cursor",
        aliases: &[],
        version_args: &["--version"],
        usage_args: &[], // No CLI usage command
    }
}

/// Returns the Cursor configuration directory.
#[allow(dead_code)]
pub fn cursor_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| h.join("Library").join("Application Support").join("Cursor"))
    }

    #[cfg(target_os = "linux")]
    {
        dirs::config_dir().map(|c| c.join("Cursor"))
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|c| c.join("Cursor"))
    }
}
