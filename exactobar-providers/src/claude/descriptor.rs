//! Claude provider descriptor.

use exactobar_core::{IconStyle, ProviderBranding, ProviderColor, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};
use std::path::PathBuf;

use crate::descriptor::{CliConfig, FetchPlan, ProviderDescriptor, TokenCostConfig};
use super::strategies::{ClaudeCliStrategy, ClaudeOAuthStrategy, ClaudePtyStrategy, ClaudeWebStrategy};

/// Creates the Claude provider descriptor.
pub fn claude_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderKind::Claude,
        metadata: claude_metadata(),
        branding: claude_branding(),
        token_cost: claude_token_cost(),
        fetch_plan: claude_fetch_plan(),
        cli: claude_cli_config(),
    }
}

/// Claude metadata configuration.
fn claude_metadata() -> ProviderMetadata {
    ProviderMetadata {
        id: ProviderKind::Claude,
        display_name: "Claude".to_string(),
        session_label: "Session".to_string(),
        weekly_label: "Weekly".to_string(),
        opus_label: Some("Opus".to_string()),
        supports_opus: true,
        supports_credits: false,
        credits_hint: String::new(),
        toggle_title: "Show Claude usage".to_string(),
        cli_name: "claude".to_string(),
        default_enabled: true,
        is_primary_provider: true,
        uses_account_fallback: false,
        dashboard_url: Some("https://claude.ai/settings/usage".to_string()),
        subscription_dashboard_url: Some("https://claude.ai/settings/billing".to_string()),
        status_page_url: Some("https://status.anthropic.com/api/v2/status.json".to_string()),
        status_link_url: Some("https://status.anthropic.com".to_string()),
    }
}

/// Claude branding configuration.
fn claude_branding() -> ProviderBranding {
    ProviderBranding {
        icon_style: IconStyle::Claude,
        icon_resource_name: "icon_claude".to_string(),
        color: ProviderColor::new(0.82, 0.58, 0.44), // Claude tan/orange
    }
}

/// Claude token cost configuration.
fn claude_token_cost() -> TokenCostConfig {
    TokenCostConfig {
        supports_token_cost: true,
        log_directory: Some(claude_log_directory),
    }
}

/// Returns the Claude log directory.
fn claude_log_directory() -> Option<PathBuf> {
    // Claude CLI stores logs in ~/.claude/logs
    dirs::home_dir().map(|h| h.join(".claude").join("logs"))
}

/// Claude fetch plan.
fn claude_fetch_plan() -> FetchPlan {
    FetchPlan {
        source_modes: vec![SourceMode::OAuth, SourceMode::CLI, SourceMode::Web],
        build_pipeline: build_claude_pipeline,
    }
}

/// Builds the Claude fetch pipeline.
fn build_claude_pipeline(ctx: &FetchContext) -> FetchPipeline {
    let mut strategies: Vec<Box<dyn exactobar_fetch::FetchStrategy>> = Vec::new();

    // OAuth strategy (highest priority)
    if ctx.settings.source_mode.allows_oauth() {
        strategies.push(Box::new(ClaudeOAuthStrategy::new()));
    }

    // CLI strategy (legacy)
    if ctx.settings.source_mode.allows_cli() {
        strategies.push(Box::new(ClaudeCliStrategy::new()));
    }

    // Web cookie strategy
    if ctx.settings.source_mode.allows_web() {
        strategies.push(Box::new(ClaudeWebStrategy::new()));
    }

    // PTY strategy (fallback)
    if ctx.settings.source_mode.allows_cli() {
        strategies.push(Box::new(ClaudePtyStrategy::new()));
    }

    FetchPipeline::with_strategies(strategies)
}

/// Claude CLI configuration.
fn claude_cli_config() -> CliConfig {
    CliConfig {
        name: "claude",
        aliases: &[],
        version_args: &["--version"],
        usage_args: &["usage"],
    }
}
