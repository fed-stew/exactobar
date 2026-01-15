//! Config command - manage configuration.

use anyhow::Result;
use clap::{Args, Subcommand};
use exactobar_providers::ProviderRegistry;
use exactobar_store::{default_config_dir, default_settings_path, SettingsStore};
use tracing::info;

use crate::output::JsonFormatter;
use crate::{Cli, OutputFormat};

/// Arguments for the config command.
#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// Config subcommands.
#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration.
    Show,

    /// Show configuration paths.
    Path,

    /// Enable a provider.
    Enable {
        /// Provider to enable.
        provider: String,
    },

    /// Disable a provider.
    Disable {
        /// Provider to disable.
        provider: String,
    },

    /// Set refresh cadence.
    Refresh {
        /// Cadence: manual, 1m, 2m, 5m, 15m.
        cadence: String,
    },

    /// Reset to defaults.
    Reset,
}

/// Runs the config command.
pub async fn run(args: &ConfigArgs, cli: &Cli) -> Result<()> {
    match &args.action {
        ConfigAction::Show => show_config(cli).await,
        ConfigAction::Path => show_paths(cli),
        ConfigAction::Enable { provider } => enable_provider(provider, cli).await,
        ConfigAction::Disable { provider } => disable_provider(provider, cli).await,
        ConfigAction::Refresh { cadence } => set_refresh(cadence, cli).await,
        ConfigAction::Reset => reset_config(cli).await,
    }
}

async fn show_config(cli: &Cli) -> Result<()> {
    let store = SettingsStore::load_default().await?;
    let settings = store.get().await;

    match cli.format {
        OutputFormat::Text => {
            println!("ExactoBar Configuration");
            println!("{}", "─".repeat(40));
            println!();
            println!("Enabled providers:");
            for provider in &settings.enabled_providers {
                if let Some(desc) = ProviderRegistry::get(*provider) {
                    println!("  • {}", desc.display_name());
                }
            }
            println!();
            println!("Refresh cadence: {}", settings.refresh_cadence);
            println!("Auto-refresh on wake: {}", settings.auto_refresh_on_wake);
            println!("Merge icons: {}", settings.merge_icons);
            println!("Debug mode: {}", settings.debug_mode);
        }
        OutputFormat::Json => {
            let formatter = JsonFormatter::new(cli.pretty);
            let output = formatter.format(&settings)?;
            println!("{}", output);
        }
    }

    Ok(())
}

fn show_paths(cli: &Cli) -> Result<()> {
    let config_dir = default_config_dir();
    let settings_path = default_settings_path();

    match cli.format {
        OutputFormat::Text => {
            println!("Configuration Paths");
            println!("{}", "─".repeat(40));
            println!();
            println!("Config dir:    {}", config_dir.display());
            println!("Settings file: {}", settings_path.display());
        }
        OutputFormat::Json => {
            let paths = serde_json::json!({
                "config_dir": config_dir.display().to_string(),
                "settings_file": settings_path.display().to_string(),
            });
            let formatter = JsonFormatter::new(cli.pretty);
            println!("{}", formatter.format(&paths)?);
        }
    }

    Ok(())
}

async fn enable_provider(name: &str, _cli: &Cli) -> Result<()> {
    let desc = ProviderRegistry::get_by_cli_name(name)
        .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", name))?;

    let store = SettingsStore::load_default().await?;
    store.set_provider_enabled(desc.id, true).await;
    store.save().await?;

    info!(provider = %desc.display_name(), "Provider enabled");
    println!("Enabled: {}", desc.display_name());

    Ok(())
}

async fn disable_provider(name: &str, _cli: &Cli) -> Result<()> {
    let desc = ProviderRegistry::get_by_cli_name(name)
        .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", name))?;

    let store = SettingsStore::load_default().await?;
    store.set_provider_enabled(desc.id, false).await;
    store.save().await?;

    info!(provider = %desc.display_name(), "Provider disabled");
    println!("Disabled: {}", desc.display_name());

    Ok(())
}

async fn set_refresh(cadence: &str, _cli: &Cli) -> Result<()> {
    use exactobar_store::RefreshCadence;

    let cadence = match cadence.to_lowercase().as_str() {
        "manual" => RefreshCadence::Manual,
        "1m" | "1" | "one" => RefreshCadence::OneMinute,
        "2m" | "2" | "two" => RefreshCadence::TwoMinutes,
        "5m" | "5" | "five" => RefreshCadence::FiveMinutes,
        "15m" | "15" | "fifteen" => RefreshCadence::FifteenMinutes,
        _ => anyhow::bail!("Unknown cadence: {}. Use: manual, 1m, 2m, 5m, 15m", cadence),
    };

    let store = SettingsStore::load_default().await?;
    store.set_refresh_cadence(cadence).await;
    store.save().await?;

    info!(cadence = %cadence, "Refresh cadence updated");
    println!("Refresh cadence set to: {}", cadence);

    Ok(())
}

async fn reset_config(_cli: &Cli) -> Result<()> {
    let path = default_settings_path();

    if path.exists() {
        tokio::fs::remove_file(&path).await?;
        info!(path = %path.display(), "Settings reset");
        println!("Configuration reset to defaults");
    } else {
        println!("No configuration file to reset");
    }

    Ok(())
}
