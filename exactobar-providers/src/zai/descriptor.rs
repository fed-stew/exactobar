//! Zai provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::ZaiApiStrategy;

pub fn zai_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Zai,
        metadata: zai_metadata(),
        branding: zai_branding(),
        token_cost: TokenCostConfig::default(),
        fetch_plan: zai_fetch_plan(),
        cli: zai_cli_config(),
    }
}

fn zai_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Zai,
        display_name: "z.ai".to_string(),
        session_label: "Requests".to_string(),
        weekly_label: "Monthly".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: true,
        credits_hint: "z.ai credits".to_string(),
        toggle_title: "Show z.ai usage".to_string(),
        cli_name: "zai".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: false,
        dashboard_url: Some("https://z.ai/settings".to_string()),
        subscription_dashboard_url: Some("https://z.ai/billing".to_string()),
        status_page_url: None,
        status_link_url: None,
    }
}

fn zai_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Zai,
        icon_resource_name: "icon_zai".to_string(),
        color: ProviderColor::new(0.0, 0.0, 0.0), // Black
    }
}

fn zai_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::ApiKey],
        build_pipeline: build_zai_pipeline,
    }
}

fn build_zai_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    if ctx.settings.source_mode.allows_api_key() {
        strategies.push(Box::new(ZaiApiStrategy::new()));
    }

    FetchPipeline::with_strategies(strategies)
}

fn zai_cli_config() -> CliConfig {
    CliConfig {
        name: "zai",
        aliases: &[],
        version_args: &["--version"],
        usage_args: &["usage"],
    }
}
