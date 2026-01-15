//! Summary command - combined summary of all providers.

use anyhow::Result;
use exactobar_core::{ProviderKind, UsageSnapshot};
use exactobar_fetch::{FetchContext, SourceMode};
use exactobar_providers::ProviderRegistry;
use std::collections::HashMap;
use tokio::time::Duration;
use tracing::info;

use crate::output::{JsonFormatter, TextFormatter};
use crate::{Cli, OutputFormat};

/// Runs the summary command.
pub async fn run(cli: &Cli) -> Result<()> {
    info!("Running summary");

    // Get all default-enabled providers
    let providers: Vec<ProviderKind> = ProviderRegistry::all()
        .iter()
        .filter(|d| d.metadata.default_enabled || d.metadata.is_primary_provider)
        .map(|d| d.id)
        .collect();

    let ctx = FetchContext::builder()
        .source_mode(SourceMode::Auto)
        .timeout(Duration::from_secs(30))
        .build();

    // Fetch from each provider
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

    // Output
    match cli.format {
        OutputFormat::Text => {
            let formatter = TextFormatter::new(!cli.no_color);
            println!("{}", formatter.format_summary(&results));
        }
        OutputFormat::Json => {
            let formatter = JsonFormatter::new(cli.pretty);
            let output = formatter.format_summary(&results)?;
            println!("{}", output);
        }
    }

    Ok(())
}
