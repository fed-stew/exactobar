//! Factory provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::{FactoryLocalStrategy, FactoryWebStrategy};

/// Builds the provider descriptor for Factory/Droid.
pub fn factory_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Factory,
        metadata: factory_metadata(),
        branding: factory_branding(),
        token_cost: TokenCostConfig::default(),
        fetch_plan: factory_fetch_plan(),
        cli: factory_cli_config(),
    }
}

fn factory_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Factory,
        display_name: "Droid".to_string(),
        session_label: "Session".to_string(),
        weekly_label: "Monthly".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: true,
        credits_hint: "Factory credits".to_string(),
        toggle_title: "Show Factory usage".to_string(),
        cli_name: "factory".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: false,
        dashboard_url: Some("https://app.factory.ai/settings".to_string()),
        subscription_dashboard_url: Some("https://app.factory.ai/billing".to_string()),
        status_page_url: None,
        status_link_url: None,
    }
}

fn factory_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Factory,
        icon_resource_name: "icon_factory".to_string(),
        color: ProviderColor::new(0.95, 0.45, 0.0), // Factory orange
    }
}

fn factory_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::Web, SourceMode::Auto],
        build_pipeline: build_factory_pipeline,
    }
}

fn build_factory_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    if ctx.settings.source_mode.allows_web() {
        strategies.push(Box::new(FactoryWebStrategy::new()));
    }

    strategies.push(Box::new(FactoryLocalStrategy::new()));

    FetchPipeline::with_strategies(strategies)
}

fn factory_cli_config() -> CliConfig {
    CliConfig {
        name: "factory",
        aliases: &["droid"],
        version_args: &["--version"],
        usage_args: &[],
    }
}
