//! Check command implementation.

use anyhow::Result;
use exactobar_store::AppState;

use crate::OutputFormat;

/// Run the check command.
pub async fn run(
    provider: Option<String>,
    format: OutputFormat,
    state: &AppState,
) -> Result<()> {
    println!("🔍 Checking provider usage...");

    if let Some(name) = provider {
        println!("  Provider: {}", name);
        // TODO: Implement single provider check
    } else {
        println!("  Checking all configured providers...");
        // TODO: Implement all providers check
    }

    println!("\n⚠️  Not yet implemented - coming soon!");

    Ok(())
}
