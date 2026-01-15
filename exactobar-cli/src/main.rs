// Lint configuration for this crate
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

//! ExactoBar CLI - LLM provider usage monitoring from the command line.
//!
//! # Examples
//!
//! ```bash
//! # Show usage for default providers (Codex + Claude)
//! exactobar
//!
//! # Show usage for a specific provider
//! exactobar --provider codex
//!
//! # Show usage for all providers
//! exactobar --provider all
//!
//! # JSON output
//! exactobar --format json --pretty
//!
//! # Force CLI source
//! exactobar usage --source cli
//!
//! # Token cost report
//! exactobar cost --provider codex
//!
//! # List providers
//! exactobar providers
//!
//! # Watch mode
//! exactobar watch --interval 30
//! ```

mod commands;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use commands::{config, cost, providers, summary, usage, watch};

// ============================================================================
// CLI Definition
// ============================================================================

/// ExactoBar CLI - LLM provider usage monitoring.
#[derive(Parser)]
#[command(name = "exactobar")]
#[command(about = "LLM provider usage monitoring CLI")]
#[command(long_about = r#"
ExactoBar monitors LLM provider usage and costs.

Supported providers:
  • Claude Code (claude)
  • OpenAI Codex (codex)
  • GitHub Copilot (copilot)
  • Cursor (cursor)
  • Google Gemini (gemini)
  • Vertex AI (vertexai)
  • Factory/Droid (factory)
  • z.ai (zai)
  • Augment (augment)
  • Kiro (kiro)
  • Antigravity (antigravity)
  • MiniMax (minimax)

Examples:
  exactobar                      # Default providers (Codex + Claude)
  exactobar --provider all       # All providers
  exactobar --provider codex     # Single provider
  exactobar --format json        # JSON output
  exactobar cost                 # Token cost report
"#)]
#[command(version)]
#[command(author = "ExactoBar Contributors")]
pub struct Cli {
    /// Subcommand to run. If none, runs 'usage' by default.
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Output format (text or json).
    #[arg(long, short = 'f', default_value = "text", global = true)]
    pub format: OutputFormat,

    /// Pretty-print JSON output.
    #[arg(long, global = true)]
    pub pretty: bool,

    /// Provider to query (or "all", "both" for multiple).
    /// Can be comma-separated: "codex,claude"
    #[arg(long, short, global = true)]
    pub provider: Option<String>,

    /// Include provider status indicators.
    #[arg(long, global = true)]
    pub status: bool,

    /// Verbose output (show debug info).
    #[arg(long, short, global = true)]
    pub verbose: bool,

    /// Disable colored output.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Quiet mode (minimal output).
    #[arg(long, short, global = true)]
    pub quiet: bool,
}

/// CLI commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Fetch current usage (default if no command specified).
    #[command(visible_alias = "u")]
    Usage(usage::UsageArgs),

    /// Show local token cost report.
    #[command(visible_alias = "c")]
    Cost(cost::CostArgs),

    /// List available providers.
    #[command(visible_alias = "p")]
    Providers,

    /// Show combined summary of all providers.
    #[command(visible_alias = "s")]
    Summary,

    /// Watch for changes (like htop for LLM usage).
    #[command(visible_alias = "w")]
    Watch(watch::WatchArgs),

    /// Manage configuration.
    Config(config::ConfigArgs),

    /// Check provider health/availability.
    Check(CheckArgs),
}

/// Arguments for check command.
#[derive(clap::Args)]
pub struct CheckArgs {
    /// Provider to check.
    #[arg(long, short)]
    pub provider: Option<String>,
}

/// Output format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable text with colors.
    #[default]
    Text,
    /// JSON output for scripting.
    Json,
}

/// CLI exit codes.
#[repr(i32)]
pub enum ExitCode {
    /// Success.
    Success = 0,
    /// General error.
    Error = 1,
    /// Provider not found or not installed.
    ProviderMissing = 2,
    /// Parse error.
    ParseError = 3,
    /// Timeout.
    Timeout = 4,
}

// ============================================================================
// Logging Setup
// ============================================================================

fn setup_logging(verbose: bool, quiet: bool) {
    if quiet {
        return; // No logging in quiet mode
    }

    let filter = if verbose {
        EnvFilter::new("exactobar=debug,info")
    } else {
        EnvFilter::new("exactobar=warn")
    };

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(false)
                .without_time()
                .with_writer(std::io::stderr),
        )
        .with(filter)
        .init();
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    setup_logging(cli.verbose, cli.quiet);

    let result = match &cli.command {
        Some(Commands::Usage(args)) => usage::run(args, &cli).await,
        Some(Commands::Cost(args)) => cost::run(args, &cli).await,
        Some(Commands::Providers) => providers::run(&cli).await,
        Some(Commands::Summary) => summary::run(&cli).await,
        Some(Commands::Watch(args)) => watch::run(args, &cli).await,
        Some(Commands::Config(args)) => config::run(args, &cli).await,
        Some(Commands::Check(args)) => run_check(args, &cli).await,
        None => {
            // Default to usage command
            usage::run(&usage::UsageArgs::default(), &cli).await
        }
    };

    if let Err(e) = result {
        if !cli.quiet {
            eprintln!("Error: {}", e);
        }
        std::process::exit(ExitCode::Error as i32);
    }

    Ok(())
}

/// Runs the check command.
async fn run_check(args: &CheckArgs, cli: &Cli) -> Result<()> {
    use exactobar_providers::ProviderRegistry;

    let providers = match &args.provider {
        Some(name) => {
            if let Some(desc) = ProviderRegistry::get_by_cli_name(name) {
                vec![desc.id]
            } else {
                anyhow::bail!("Unknown provider: {}", name);
            }
        }
        None => ProviderRegistry::kinds(),
    };

    let ctx = exactobar_fetch::FetchContext::builder().build();

    for provider in providers {
        let desc = ProviderRegistry::get(provider).unwrap();
        let pipeline = desc.build_pipeline(&ctx);

        // Check if pipeline has any available strategies
        // For now, we just check if the pipeline can execute
        let outcome = pipeline.execute(&ctx).await;
        let available: Vec<String> = match &outcome.result {
            Ok(fetch_result) => vec![fetch_result.strategy_id.clone()],
            Err(_) => vec![],
        };

        if cli.format == OutputFormat::Json {
            println!(
                r#"{{"provider":"{}","available":{},"strategies":{}}}"#,
                desc.cli_name(),
                !available.is_empty(),
                serde_json::to_string(&available)?
            );
        } else {
            let status = if available.is_empty() {
                if cli.no_color {
                    "✗ Not available".to_string()
                } else {
                    "\x1b[31m✗ Not available\x1b[0m".to_string()
                }
            } else {
                if cli.no_color {
                    format!("✓ {} strategies", available.len())
                } else {
                    format!("\x1b[32m✓\x1b[0m {} strategies", available.len())
                }
            };

            println!("{:<15} {}", desc.display_name(), status);

            if cli.verbose && !available.is_empty() {
                for s in &available {
                    println!("  - {}", s);
                }
            }
        }
    }

    Ok(())
}
