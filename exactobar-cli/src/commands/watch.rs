//! Watch command - real-time usage monitoring.

use anyhow::Result;
use clap::Args;
use exactobar_core::{ProviderKind, UsageSnapshot};
use exactobar_fetch::{FetchContext, SourceMode};
use exactobar_providers::ProviderRegistry;
use std::collections::HashMap;
use std::io::{stdout, Write};
use tokio::time::{Duration, interval};
use tracing::info;

use crate::output::TextFormatter;
use crate::Cli;

/// Arguments for watch command.
#[derive(Args)]
pub struct WatchArgs {
    /// Refresh interval in seconds.
    #[arg(long, short, default_value = "30")]
    pub interval: u64,

    /// Provider to watch.
    #[arg(long, short)]
    pub provider: Option<String>,

    /// Minimum interval to use.
    #[arg(long, default_value = "10")]
    pub min_interval: u64,
}

/// Runs the watch command.
pub async fn run(args: &WatchArgs, cli: &Cli) -> Result<()> {
    let refresh_interval = args.interval.max(args.min_interval);

    info!(interval = refresh_interval, "Starting watch mode");

    // Determine providers
    let providers = match &args.provider {
        Some(name) if name == "all" => ProviderRegistry::kinds(),
        Some(name) => {
            if let Some(desc) = ProviderRegistry::get_by_cli_name(name) {
                vec![desc.id]
            } else {
                anyhow::bail!("Unknown provider: {}", name);
            }
        }
        None => vec![ProviderKind::Codex, ProviderKind::Claude],
    };

    let ctx = FetchContext::builder()
        .source_mode(SourceMode::Auto)
        .timeout(Duration::from_secs(30))
        .build();

    let formatter = TextFormatter::new(!cli.no_color);

    let mut ticker = interval(Duration::from_secs(refresh_interval));

    // Initial fetch
    ticker.tick().await;

    loop {
        // Clear screen
        print!("\x1b[2J\x1b[H");
        stdout().flush()?;

        // Header
        let now = chrono::Local::now();
        println!(
            "ExactoBar Watch Mode - {} (refresh: {}s)",
            now.format("%H:%M:%S"),
            refresh_interval
        );
        println!("{}", "─".repeat(50));
        println!();

        // Fetch each provider
        let mut results: HashMap<ProviderKind, Option<UsageSnapshot>> = HashMap::new();

        for provider in &providers {
            if let Some(desc) = ProviderRegistry::get(*provider) {
                let pipeline = desc.build_pipeline(&ctx);
                let outcome = pipeline.execute(&ctx).await;

                match outcome.result {
                    Ok(fetch_result) => {
                        results.insert(*provider, Some(fetch_result.snapshot));
                    }
                    Err(_) => {
                        results.insert(*provider, None);
                    }
                }
            }
        }

        // Display results
        println!("{}", formatter.format_summary(&results));
        println!();
        println!("Press Ctrl+C to exit");

        // Wait for next tick
        ticker.tick().await;
    }
}
