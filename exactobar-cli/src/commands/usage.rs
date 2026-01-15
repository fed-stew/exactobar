//! Usage command - fetch and display provider usage.

use anyhow::Result;
use clap::Args;
use exactobar_core::{ProviderKind, UsageSnapshot};
use exactobar_fetch::{FetchContext, SourceMode};
use exactobar_providers::ProviderRegistry;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::output::{JsonFormatter, TextFormatter};
use crate::{Cli, ExitCode, OutputFormat};

/// Arguments for the usage command.
#[derive(Args, Default)]
pub struct UsageArgs {
    /// Provider to query (or "all", "both" for multiple).
    /// Can be comma-separated: "codex,claude"
    #[arg(long, short)]
    pub provider: Option<String>,

    /// Hide credits in output.
    #[arg(long)]
    pub no_credits: bool,

    /// Web timeout in seconds.
    #[arg(long, default_value = "60")]
    pub web_timeout: u64,

    /// Source mode for fetching (auto, cli, oauth, api, web).
    #[arg(long, default_value = "auto")]
    pub source: String,

    /// Show raw debug output.
    #[arg(long)]
    pub debug: bool,
}

/// Runs the usage command.
pub async fn run(args: &UsageArgs, cli: &Cli) -> Result<()> {
    // Determine which providers to query
    let provider_arg = args.provider.as_ref().or(cli.provider.as_ref());
    let providers = parse_provider_selection(provider_arg)?;

    info!(providers = ?providers, "Fetching usage");

    // Create fetch context
    let source_mode = parse_source_mode(&args.source)?;
    let ctx = FetchContext::builder()
        .source_mode(source_mode)
        .timeout(std::time::Duration::from_secs(args.web_timeout))
        .build();

    // Fetch usage from each provider (in parallel if multiple)
    let results = fetch_all(&providers, &ctx).await;

    // Check for any successful results
    let has_success = results.values().any(|r| r.is_ok());

    // Format and output
    output_results(&results, args, cli)?;

    // Exit code based on results
    if !has_success {
        std::process::exit(ExitCode::ProviderMissing as i32);
    }

    Ok(())
}

/// Fetches usage from all providers.
async fn fetch_all(
    providers: &[ProviderKind],
    ctx: &FetchContext,
) -> HashMap<ProviderKind, Result<UsageSnapshot, String>> {
    

    // Note: This runs sequentially because FetchContext isn't Clone.
    // For true parallelism, we'd need to restructure the context.
    let mut results = HashMap::new();
    for provider in providers {
        let result = fetch_one(*provider, ctx).await;
        results.insert(*provider, result);
    }

    results
}

/// Fetches usage from a single provider.
async fn fetch_one(
    provider: ProviderKind,
    ctx: &FetchContext,
) -> Result<UsageSnapshot, String> {
    let desc = ProviderRegistry::get(provider)
        .ok_or_else(|| format!("Provider {:?} not found", provider))?;

    debug!(provider = ?provider, "Building pipeline");

    let pipeline = desc.build_pipeline(ctx);
    let outcome = pipeline.execute(ctx).await;

    match outcome.result {
        Ok(fetch_result) => {
            debug!(
                provider = ?provider,
                strategy = ?fetch_result.strategy_id,
                "Fetch successful"
            );
            Ok(fetch_result.snapshot)
        }
        Err(e) => {
            warn!(provider = ?provider, error = %e, "Fetch failed");
            Err(e.to_string())
        }
    }
}

/// Parses provider selection from argument.
fn parse_provider_selection(arg: Option<&String>) -> Result<Vec<ProviderKind>> {
    match arg.map(|s| s.to_lowercase()).as_deref() {
        None | Some("both") | Some("default") => {
            // Default: Codex and Claude (primary providers)
            Ok(vec![ProviderKind::Codex, ProviderKind::Claude])
        }
        Some("all") => {
            // All registered providers
            Ok(ProviderRegistry::kinds())
        }
        Some(names) => {
            // Could be comma-separated
            let mut providers = Vec::new();
            for name in names.split(',') {
                let name = name.trim();
                if let Some(desc) = ProviderRegistry::get_by_cli_name(name) {
                    providers.push(desc.id);
                } else {
                    anyhow::bail!("Unknown provider: {}", name);
                }
            }
            if providers.is_empty() {
                anyhow::bail!("No valid providers specified");
            }
            Ok(providers)
        }
    }
}

/// Parses source mode from string.
fn parse_source_mode(s: &str) -> Result<SourceMode> {
    match s.to_lowercase().as_str() {
        "auto" => Ok(SourceMode::Auto),
        "cli" => Ok(SourceMode::CLI),
        "oauth" => Ok(SourceMode::OAuth),
        "api" | "apikey" | "api_key" => Ok(SourceMode::ApiKey),
        "web" | "cookies" => Ok(SourceMode::Web),

        _ => anyhow::bail!("Unknown source mode: {}. Valid options: auto, cli, oauth, api, web, local, rpc", s),
    }
}

/// Outputs results in the appropriate format.
fn output_results(
    results: &HashMap<ProviderKind, Result<UsageSnapshot, String>>,
    args: &UsageArgs,
    cli: &Cli,
) -> Result<()> {
    match cli.format {
        OutputFormat::Text => {
            let formatter = TextFormatter::new(!cli.no_color);

            // Sort providers for consistent output
            let mut sorted: Vec<_> = results.iter().collect();
            sorted.sort_by_key(|(k, _)| format!("{:?}", k));

            let mut first = true;
            for (provider, result) in sorted {
                if !first {
                    println!(); // Blank line between providers
                }
                first = false;

                let desc = ProviderRegistry::get(*provider);
                match result {
                    Ok(snapshot) => {
                        let output = formatter.format_usage(snapshot, desc, !args.no_credits);
                        println!("{}", output);
                    }
                    Err(e) => {
                        let name = desc.map(|d| d.display_name()).unwrap_or("Unknown");
                        println!("{}", formatter.format_error(name, e));
                    }
                }
            }
        }
        OutputFormat::Json => {
            let formatter = JsonFormatter::new(cli.pretty);
            let output = formatter.format_results(results)?;
            println!("{}", output);
        }
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_provider_default() {
        let providers = parse_provider_selection(None).unwrap();
        assert!(providers.contains(&ProviderKind::Codex));
        assert!(providers.contains(&ProviderKind::Claude));
    }

    #[test]
    fn test_parse_provider_all() {
        let providers = parse_provider_selection(Some(&"all".to_string())).unwrap();
        assert!(providers.len() >= 2);
    }

    #[test]
    fn test_parse_provider_single() {
        let providers = parse_provider_selection(Some(&"codex".to_string())).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0], ProviderKind::Codex);
    }

    #[test]
    fn test_parse_provider_comma_separated() {
        let providers = parse_provider_selection(Some(&"codex,claude".to_string())).unwrap();
        assert_eq!(providers.len(), 2);
    }

    #[test]
    fn test_parse_source_mode() {
        assert!(matches!(parse_source_mode("auto").unwrap(), SourceMode::Auto));
        assert!(matches!(parse_source_mode("cli").unwrap(), SourceMode::CLI));
        assert!(matches!(parse_source_mode("oauth").unwrap(), SourceMode::OAuth));
        assert!(matches!(parse_source_mode("web").unwrap(), SourceMode::Web));
    }

    #[test]
    fn test_parse_source_mode_invalid() {
        assert!(parse_source_mode("invalid").is_err());
    }
}
