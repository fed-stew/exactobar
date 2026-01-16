//! MiniMax provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::{
    HailuoaiWebStrategy, MiniMaxLocalStorageStrategy, MiniMaxLocalStrategy, MiniMaxWebStrategy,
};

/// Builds the provider descriptor for MiniMax.
pub fn minimax_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::MiniMax,
        metadata: minimax_metadata(),
        branding: minimax_branding(),
        token_cost: TokenCostConfig::default(),
        fetch_plan: minimax_fetch_plan(),
        cli: minimax_cli_config(),
    }
}

fn minimax_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::MiniMax,
        display_name: "MiniMax".to_string(),
        session_label: "Tokens".to_string(),
        weekly_label: "Monthly".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: true,
        credits_hint: "MiniMax credits".to_string(),
        toggle_title: "Show MiniMax usage".to_string(),
        cli_name: "minimax".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: false,
        dashboard_url: Some("https://hailuoai.com/settings".to_string()),
        subscription_dashboard_url: Some("https://hailuoai.com/pricing".to_string()),
        status_page_url: None,
        status_link_url: None,
    }
}

fn minimax_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::MiniMax,
        icon_resource_name: "icon_minimax".to_string(),
        color: ProviderColor::new(0.0, 0.8, 0.6), // Teal
    }
}

fn minimax_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::Web, SourceMode::Auto],
        build_pipeline: build_minimax_pipeline,
    }
}

fn build_minimax_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    if ctx.settings.source_mode.allows_web() {
        // Primary: minimax.chat cookies
        strategies.push(Box::new(MiniMaxWebStrategy::new()));
        // Secondary: hailuoai.com cookies (MiniMax's web interface)
        strategies.push(Box::new(HailuoaiWebStrategy::new()));
    }

    // Tertiary: browser localStorage tokens
    strategies.push(Box::new(MiniMaxLocalStorageStrategy::new()));

    // Fallback: local config file
    strategies.push(Box::new(MiniMaxLocalStrategy::new()));

    FetchPipeline::with_strategies(strategies)
}

fn minimax_cli_config() -> CliConfig {
    CliConfig {
        name: "minimax",
        aliases: &[],
        version_args: &["--version"],
        usage_args: &[],
    }
}
