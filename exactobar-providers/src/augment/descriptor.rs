//! Augment provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::AugmentWebStrategy;

pub fn augment_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Augment,
        metadata: augment_metadata(),
        branding: augment_branding(),
        token_cost: TokenCostConfig::default(),
        fetch_plan: augment_fetch_plan(),
        cli: augment_cli_config(),
    }
}

fn augment_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Augment,
        display_name: "Augment".to_string(),
        session_label: "Credits".to_string(),
        weekly_label: "Monthly".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: true,
        credits_hint: "Augment credits".to_string(),
        toggle_title: "Show Augment usage".to_string(),
        cli_name: "augment".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: false,
        dashboard_url: Some("https://augmentcode.com/settings".to_string()),
        subscription_dashboard_url: Some("https://augmentcode.com/billing".to_string()),
        status_page_url: None,
        status_link_url: None,
    }
}

fn augment_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Augment,
        icon_resource_name: "icon_augment".to_string(),
        color: ProviderColor::new(0.56, 0.27, 0.68), // Purple
    }
}

fn augment_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::Web],
        build_pipeline: build_augment_pipeline,
    }
}

fn build_augment_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    if ctx.settings.source_mode.allows_web() {
        strategies.push(Box::new(AugmentWebStrategy::new()));
    }

    FetchPipeline::with_strategies(strategies)
}

fn augment_cli_config() -> CliConfig {
    CliConfig {
        name: "augment",
        aliases: &[],
        version_args: &["--version"],
        usage_args: &[],
    }
}
