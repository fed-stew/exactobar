//! Copilot provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::{CopilotApiStrategy, CopilotEnvStrategy};

pub fn copilot_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Copilot,
        metadata: copilot_metadata(),
        branding: copilot_branding(),
        token_cost: TokenCostConfig::default(),
        fetch_plan: copilot_fetch_plan(),
        cli: copilot_cli_config(),
    }
}

fn copilot_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Copilot,
        display_name: "Copilot".to_string(),
        session_label: "Completions".to_string(),
        weekly_label: "Monthly".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: false,
        credits_hint: String::new(),
        toggle_title: "Show Copilot usage".to_string(),
        cli_name: "copilot".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: false,
        dashboard_url: Some("https://github.com/settings/copilot".to_string()),
        subscription_dashboard_url: Some("https://github.com/settings/billing".to_string()),
        status_page_url: Some("https://www.githubstatus.com/api/v2/status.json".to_string()),
        status_link_url: Some("https://www.githubstatus.com".to_string()),
    }
}

fn copilot_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Copilot,
        icon_resource_name: "icon_copilot".to_string(),
        color: ProviderColor::new(0.14, 0.14, 0.14), // GitHub dark
    }
}

fn copilot_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::OAuth, SourceMode::ApiKey],
        build_pipeline: build_copilot_pipeline,
    }
}

fn build_copilot_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    if ctx.settings.source_mode.allows_oauth() {
        strategies.push(Box::new(CopilotApiStrategy::new()));
    }

    if ctx.settings.source_mode.allows_api_key() {
        strategies.push(Box::new(CopilotEnvStrategy::new()));
    }

    FetchPipeline::with_strategies(strategies)
}

fn copilot_cli_config() -> CliConfig {
    CliConfig {
        name: "gh",
        aliases: &["copilot"],
        version_args: &["--version"],
        usage_args: &["copilot", "usage"],
    }
}
