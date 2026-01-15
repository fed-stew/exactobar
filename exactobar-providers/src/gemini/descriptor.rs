//! Gemini provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::{GeminiCliStrategy, GeminiOAuthStrategy};

/// Creates the Gemini provider descriptor.
pub fn gemini_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Gemini,
        metadata: gemini_metadata(),
        branding: gemini_branding(),
        token_cost: TokenCostConfig::default(),
        fetch_plan: gemini_fetch_plan(),
        cli: gemini_cli_config(),
    }
}

fn gemini_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Gemini,
        display_name: "Gemini".to_string(),
        session_label: "Requests".to_string(),
        weekly_label: "Daily".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: false,
        credits_hint: String::new(),
        toggle_title: "Show Gemini usage".to_string(),
        cli_name: "gemini".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: true,
        dashboard_url: Some("https://aistudio.google.com/app/usage".to_string()),
        subscription_dashboard_url: None,
        status_page_url: None,
        status_link_url: Some("https://status.cloud.google.com".to_string()),
    }
}

fn gemini_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Gemini,
        icon_resource_name: "icon_gemini".to_string(),
        color: ProviderColor::new(0.25, 0.52, 0.96), // Google blue
    }
}

fn gemini_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::OAuth, SourceMode::CLI],
        build_pipeline: build_gemini_pipeline,
    }
}

fn build_gemini_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    if ctx.settings.source_mode.allows_oauth() {
        strategies.push(Box::new(GeminiOAuthStrategy::new()));
    }

    if ctx.settings.source_mode.allows_cli() {
        strategies.push(Box::new(GeminiCliStrategy::new()));
    }

    FetchPipeline::with_strategies(strategies)
}

fn gemini_cli_config() -> CliConfig {
    CliConfig {
        name: "gemini",
        aliases: &["gcloud"],
        version_args: &["--version"],
        usage_args: &["usage"],
    }
}
