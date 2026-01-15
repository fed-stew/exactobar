//! Cost command - show local token cost report.
//!
//! Scans local log files for token usage and calculates costs.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use clap::Args;
use exactobar_core::ProviderKind;
use exactobar_providers::ProviderRegistry;
use exactobar_store::{CostUsageSnapshot, DailyCost};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::output::{JsonFormatter, TextFormatter};
use crate::{Cli, OutputFormat};

/// Arguments for the cost command.
#[derive(Args)]
pub struct CostArgs {
    /// Provider for cost report.
    #[arg(long, short, default_value = "all")]
    pub provider: String,

    /// Refresh (re-scan logs, ignore cache).
    #[arg(long)]
    pub refresh: bool,

    /// Number of days to include.
    #[arg(long, default_value = "30")]
    pub days: u32,

    /// Show daily breakdown.
    #[arg(long)]
    pub daily: bool,
}

/// Runs the cost command.
pub async fn run(args: &CostArgs, cli: &Cli) -> Result<()> {
    info!(provider = %args.provider, refresh = args.refresh, "Running cost report");

    // Determine which providers to scan
    let providers = parse_cost_providers(&args.provider)?;

    // Scan logs for each provider
    let mut results: HashMap<ProviderKind, CostUsageSnapshot> = HashMap::new();

    for provider in &providers {
        let desc = ProviderRegistry::get(*provider);
        if desc.is_none() {
            continue;
        }

        let desc = desc.unwrap();
        if !desc.token_cost.supports_token_cost {
            continue;
        }

        // Get log directory
        if let Some(log_dir_fn) = desc.token_cost.log_directory {
            if let Some(log_dir) = log_dir_fn() {
                if log_dir.exists() {
                    debug!(provider = ?provider, dir = %log_dir.display(), "Scanning logs");

                    let snapshot = scan_logs(&log_dir, args.days)?;
                    results.insert(*provider, snapshot);
                } else {
                    debug!(provider = ?provider, "Log directory not found");
                }
            }
        }
    }

    // Output results
    output_cost_results(&results, args, cli)?;

    Ok(())
}

/// Scans log files and aggregates token usage.
fn scan_logs(log_dir: &PathBuf, days: u32) -> Result<CostUsageSnapshot> {
    let mut total_tokens: u64 = 0;
    let mut total_cost: f64 = 0.0;
    let mut daily_map: HashMap<NaiveDate, (u64, f64)> = HashMap::new();

    let cutoff = Utc::now() - chrono::Duration::days(days as i64);

    // Read all .jsonl files
    let entries = fs::read_dir(log_dir)?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process .jsonl files
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        // Read and parse
        match fs::read_to_string(&path) {
            Ok(content) => {
                for line in content.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }

                    if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                        // Check date cutoff
                        if let Some(timestamp) = &entry.timestamp {
                            if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
                                if dt < cutoff {
                                    continue;
                                }

                                let date = dt.date_naive();
                                let tokens = entry.total_tokens();
                                let cost = entry.cost_usd.unwrap_or(0.0);

                                total_tokens += tokens;
                                total_cost += cost;

                                let entry = daily_map.entry(date).or_insert((0, 0.0));
                                entry.0 += tokens;
                                entry.1 += cost;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!(path = %path.display(), error = %e, "Failed to read log file");
            }
        }
    }

    // Build daily breakdown
    let mut daily: Vec<DailyCost> = daily_map
        .into_iter()
        .map(|(date, (tokens, cost))| DailyCost {
            date: date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            tokens,
            cost_usd: cost,
        })
        .collect();

    daily.sort_by_key(|d| d.date);

    Ok(CostUsageSnapshot {
        total_tokens,
        total_cost_usd: total_cost,
        daily,
        scanned_at: Some(Utc::now()),
    })
}

/// Log entry structure (generic for multiple providers).
#[derive(Debug, Deserialize)]
struct LogEntry {
    #[serde(default)]
    timestamp: Option<String>,

    #[serde(default, alias = "input_tokens")]
    input_tokens: Option<u64>,

    #[serde(default, alias = "output_tokens")]
    output_tokens: Option<u64>,

    #[serde(default, alias = "total_tokens")]
    total_tokens: Option<u64>,

    #[serde(default)]
    cost_usd: Option<f64>,
}

impl LogEntry {
    fn total_tokens(&self) -> u64 {
        self.total_tokens.unwrap_or_else(|| {
            self.input_tokens.unwrap_or(0) + self.output_tokens.unwrap_or(0)
        })
    }
}

/// Parses provider selection for cost command.
fn parse_cost_providers(arg: &str) -> Result<Vec<ProviderKind>> {
    match arg.to_lowercase().as_str() {
        "all" => {
            // Only providers that support token cost
            Ok(ProviderRegistry::all()
                .iter()
                .filter(|d| d.token_cost.supports_token_cost)
                .map(|d| d.id)
                .collect())
        }
        name => {
            if let Some(desc) = ProviderRegistry::get_by_cli_name(name) {
                if desc.token_cost.supports_token_cost {
                    Ok(vec![desc.id])
                } else {
                    anyhow::bail!("Provider {} does not support token cost tracking", name);
                }
            } else {
                anyhow::bail!("Unknown provider: {}", name);
            }
        }
    }
}

/// Outputs cost results.
fn output_cost_results(
    results: &HashMap<ProviderKind, CostUsageSnapshot>,
_args: &CostArgs,
    cli: &Cli,
) -> Result<()> {
    if results.is_empty() {
        println!("No token cost data available.");
        println!();
        println!("Token cost tracking requires log files. Supported providers:");

        for desc in ProviderRegistry::all() {
            if desc.token_cost.supports_token_cost {
                println!("  • {} ({})", desc.display_name(), desc.cli_name());
            }
        }

        return Ok(());
    }

    match cli.format {
        OutputFormat::Text => {
            let formatter = TextFormatter::new(!cli.no_color);

            let mut first = true;
            for (provider, snapshot) in results {
                if !first {
                    println!();
                }
                first = false;

                let desc = ProviderRegistry::get(*provider);
                let output = formatter.format_cost(snapshot, desc);
                println!("{}", output);
            }
        }
        OutputFormat::Json => {
            let formatter = JsonFormatter::new(cli.pretty);
            let output = formatter.format_cost_results(results)?;
            println!("{}", output);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cost_providers_all() {
        let providers = parse_cost_providers("all").unwrap();
        assert!(!providers.is_empty());
    }

    #[test]
    fn test_log_entry_total_tokens() {
        let entry = LogEntry {
            timestamp: None,
            input_tokens: Some(100),
            output_tokens: Some(50),
            total_tokens: None,
            cost_usd: None,
        };
        assert_eq!(entry.total_tokens(), 150);

        let entry_with_total = LogEntry {
            timestamp: None,
            input_tokens: Some(100),
            output_tokens: Some(50),
            total_tokens: Some(200),
            cost_usd: None,
        };
        assert_eq!(entry_with_total.total_tokens(), 200);
    }
}
