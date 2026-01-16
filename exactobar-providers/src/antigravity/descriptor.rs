//! Antigravity provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::AntigravityLocalStrategy;

/// Builds the provider descriptor for Antigravity.
pub fn antigravity_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Antigravity,
        metadata: antigravity_metadata(),
        branding: antigravity_branding(),
        token_cost: TokenCostConfig::default(),
        fetch_plan: antigravity_fetch_plan(),
        cli: antigravity_cli_config(),
    }
}

fn antigravity_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Antigravity,
        display_name: "Antigravity".to_string(),
        session_label: "Session".to_string(),
        weekly_label: "Daily".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: false,
        credits_hint: String::new(),
        toggle_title: "Show Antigravity usage".to_string(),
        cli_name: "antigravity".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: false,
        dashboard_url: None, // Local app
        subscription_dashboard_url: None,
        status_page_url: None,
        status_link_url: None,
    }
}

fn antigravity_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Antigravity,
        icon_resource_name: "icon_antigravity".to_string(),
        color: ProviderColor::new(0.5, 0.0, 0.5), // Purple
    }
}

fn antigravity_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::Auto],
        build_pipeline: build_antigravity_pipeline,
    }
}

fn build_antigravity_pipeline(_ctx: &FetchContext) -> FetchPipeline {
    let strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = vec![
        Box::new(AntigravityLocalStrategy::new()),
    ];

    FetchPipeline::with_strategies(strategies)
}

fn antigravity_cli_config() -> CliConfig {
    CliConfig {
        name: "antigravity",
        aliases: &[],
        version_args: &["--version"],
        usage_args: &[],
    }
}
