//! VertexAI provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};
use std::path::PathBuf;

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::{VertexAILocalStrategy, VertexAIOAuthStrategy};

pub fn vertexai_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::VertexAI,
        metadata: vertexai_metadata(),
        branding: vertexai_branding(),
        token_cost: vertexai_token_cost(),
        fetch_plan: vertexai_fetch_plan(),
        cli: vertexai_cli_config(),
    }
}

fn vertexai_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::VertexAI,
        display_name: "Vertex AI".to_string(),
        session_label: "Requests".to_string(),
        weekly_label: "Daily".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: false,
        credits_hint: String::new(),
        toggle_title: "Show Vertex AI usage".to_string(),
        cli_name: "vertexai".to_string(),
        default_enabled: false,
        is_primary_provider: false,
        uses_account_fallback: true,
        dashboard_url: Some("https://console.cloud.google.com/vertex-ai".to_string()),
        subscription_dashboard_url: Some("https://console.cloud.google.com/billing".to_string()),
        status_page_url: None,
        status_link_url: Some("https://status.cloud.google.com".to_string()),
    }
}

fn vertexai_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::VertexAI,
        icon_resource_name: "icon_vertexai".to_string(),
        color: ProviderColor::new(0.26, 0.52, 0.96), // Google Cloud blue
    }
}

fn vertexai_token_cost() -> TokenCostConfig {
    TokenCostConfig {
        supports_token_cost: true,
        log_directory: Some(vertexai_log_directory),
    }
}

fn vertexai_log_directory() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".config").join("gcloud").join("logs"))
}

fn vertexai_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::OAuth, SourceMode::Auto],
        build_pipeline: build_vertexai_pipeline,
    }
}

fn build_vertexai_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    if ctx.settings.source_mode.allows_oauth() {
        strategies.push(Box::new(VertexAIOAuthStrategy::new()));
    }

    strategies.push(Box::new(VertexAILocalStrategy::new()));

    FetchPipeline::with_strategies(strategies)
}

fn vertexai_cli_config() -> CliConfig {
    CliConfig {
        name: "gcloud",
        aliases: &["vertexai"],
        version_args: &["--version"],
        usage_args: &["ai", "operations", "list"],
    }
}
