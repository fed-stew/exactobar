//! Codex provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};
use std::path::PathBuf;

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::{CodexApiStrategy, CodexCliStrategy, CodexPtyStrategy, CodexRpcStrategy};

/// Creates the Codex provider descriptor.
pub fn codex_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Codex,
        metadata: codex_metadata(),
        branding: codex_branding(),
        token_cost: codex_token_cost(),
        fetch_plan: codex_fetch_plan(),
        cli: codex_cli_config(),
    }
}

/// Codex metadata configuration.
fn codex_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Codex,
        display_name: "Codex".to_string(),
        session_label: "Session".to_string(),
        weekly_label: "Weekly".to_string(),
        opus_label: None,
        supports_opus: false,
        supports_credits: true,
        credits_hint: "OpenAI API credits".to_string(),
        toggle_title: "Show Codex usage".to_string(),
        cli_name: "codex".to_string(),
        default_enabled: true,
        is_primary_provider: true,
        uses_account_fallback: true,
        dashboard_url: Some("https://platform.openai.com/usage".to_string()),
        subscription_dashboard_url: Some(
            "https://platform.openai.com/settings/organization/billing".to_string(),
        ),
        status_page_url: Some("https://status.openai.com/api/v2/status.json".to_string()),
        status_link_url: Some("https://status.openai.com".to_string()),
    }
}

/// Codex branding configuration.
fn codex_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Codex,
        icon_resource_name: "icon_codex".to_string(),
        color: ProviderColor::new(0.0, 0.64, 0.38), // OpenAI green
    }
}

/// Codex token cost configuration.
fn codex_token_cost() -> TokenCostConfig {
    TokenCostConfig {
        supports_token_cost: true,
        log_directory: Some(codex_log_directory),
    }
}

/// Returns the Codex log directory.
fn codex_log_directory() -> Option<PathBuf> {
    // Codex stores logs in ~/.codex/logs or similar
    dirs::home_dir().map(|h| h.join(".codex").join("logs"))
}

/// Codex fetch plan.
fn codex_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::CLI, SourceMode::ApiKey, SourceMode::Web],
        build_pipeline: build_codex_pipeline,
    }
}

/// Builds the Codex fetch pipeline.
fn build_codex_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    // RPC strategy (highest priority) - JSON-RPC to app-server
    if ctx.settings.source_mode.allows_cli() {
        strategies.push(Box::new(CodexRpcStrategy::new()));
    }

    // PTY strategy (fallback) - interactive /status command
    if ctx.settings.source_mode.allows_cli() {
        strategies.push(Box::new(CodexPtyStrategy::new()));
    }

    // CLI strategy - codex usage --json
    if ctx.settings.source_mode.allows_cli() {
        strategies.push(Box::new(CodexCliStrategy::new()));
    }

    // API strategy
    if ctx.settings.source_mode.allows_api_key() {
        strategies.push(Box::new(CodexApiStrategy::new()));
    }

    FetchPipeline::with_strategies(strategies)
}

/// Codex CLI configuration.
fn codex_cli_config() -> CliConfig {
    CliConfig {
        name: "codex",
        aliases: &["openai"],
        version_args: &["--version"],
        usage_args: &["usage", "--json"],
    }
}
