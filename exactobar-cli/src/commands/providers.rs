//! Providers command - list available providers.

use anyhow::Result;
use exactobar_fetch::FetchContext;
use exactobar_providers::ProviderRegistry;
use tracing::info;

use crate::output::{JsonFormatter, TextFormatter};
use crate::{Cli, OutputFormat};

/// Runs the providers command.
pub async fn run(cli: &Cli) -> Result<()> {
    info!("Listing providers");

    let providers = ProviderRegistry::all();
    let _ctx = FetchContext::builder().build();

    match cli.format {
        OutputFormat::Text => {
            let formatter = TextFormatter::new(!cli.no_color);

            println!("{}", formatter.format_providers_header());
            println!("{}", "─".repeat(70));

            for desc in providers {
                // For now, assume installed if it's a primary provider or default enabled
                let installed = desc.metadata.is_primary_provider || desc.metadata.default_enabled;
                println!("{}", formatter.format_provider_line(desc, installed));
            }

            println!();
            println!(
                "Total: {} providers ({} primary)",
                providers.len(),
                providers.iter().filter(|d| d.metadata.is_primary_provider).count()
            );
        }
        OutputFormat::Json => {
            let formatter = JsonFormatter::new(cli.pretty);
            let output = formatter.format_providers(&providers)?;
            println!("{}", output);
        }
    }

    Ok(())
}
