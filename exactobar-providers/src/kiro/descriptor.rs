//! Kiro provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::KiroCliStrategy;

pub fn kiro_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Kiro,
        metadata: kiro_metadata(),
        branding: kiro_branding(),
        token_cost: TokenCostConfig::default(),
        fetch_plan: kiro_fetch_plan(),
        cli: kiro_cli_config(),
    }
}

fn kiro_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Kiro,
        display_name: "Kiro".to_string(),
        session_label: "Credits".to_string(),
        weekly_label: "Monthly".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: true,
        credits_hint: "Kiro credits".to_string(),
        toggle_title: "Show Kiro usage".to_string(),
        cli_name: "kiro".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: false,
        dashboard_url: Some("https://kiro.ai/settings".to_string()),
        subscription_dashboard_url: Some("https://kiro.ai/billing".to_string()),
        status_page_url: None,
        status_link_url: Some("https://health.aws.amazon.com/health/status".to_string()),
    }
}

fn kiro_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Kiro,
        icon_resource_name: "icon_kiro".to_string(),
        color: ProviderColor::new(1.0, 0.6, 0.0), // AWS orange
    }
}

fn kiro_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::CLI],
        build_pipeline: build_kiro_pipeline,
    }
}

fn build_kiro_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    if ctx.settings.source_mode.allows_cli() {
        strategies.push(Box::new(KiroCliStrategy::new()));
    }

    FetchPipeline::with_strategies(strategies)
}

fn kiro_cli_config() -> CliConfig {
    CliConfig {
        name: "kiro-cli",
        aliases: &["kiro"],
        version_args: &["--version"],
        usage_args: &["/usage"],
    }
}
