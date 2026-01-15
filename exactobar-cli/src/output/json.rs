//! JSON output formatting.

use anyhow::Result;
use chrono::{DateTime, Utc};
use exactobar_core::{FetchSource, ProviderKind, UsageSnapshot, UsageWindow};
use exactobar_providers::ProviderDescriptor;
use exactobar_store::CostUsageSnapshot;
use serde::{Serialize, Serializer};
use std::collections::HashMap;

// ============================================================================
// Output Types
// ============================================================================

/// JSON output for a single provider.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOutput {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<CreditsOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Status indicator.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusOutput {
    pub indicator: String,
    pub description: String,
}

/// Usage windows.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<WindowOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<WindowOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tertiary: Option<WindowOutput>,
    #[serde(serialize_with = "serialize_datetime")]
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<IdentityOutput>,
}

/// A single usage window.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowOutput {
    pub used_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_datetime_opt")]
    pub resets_at: Option<DateTime<Utc>>,
}

/// Identity info.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_method: Option<String>,
}

/// Credits info.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditsOutput {
    pub remaining_usd: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_usd: Option<f64>,
}

/// Cost report output.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostOutput {
    pub provider: String,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub daily: Vec<DailyCostOutput>,
}

/// Daily cost entry.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyCostOutput {
    pub date: String,
    pub tokens: u64,
    pub cost_usd: f64,
}

/// Provider info output.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfoOutput {
    pub id: String,
    pub display_name: String,
    pub cli_name: String,
    pub default_enabled: bool,
    pub is_primary: bool,
    pub supports_credits: bool,
    pub supports_opus: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_page_url: Option<String>,
}

// ============================================================================
// Serialization helpers
// ============================================================================

fn serialize_datetime<S>(dt: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&dt.to_rfc3339())
}

fn serialize_datetime_opt<S>(dt: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match dt {
        Some(dt) => s.serialize_str(&dt.to_rfc3339()),
        None => s.serialize_none(),
    }
}

// ============================================================================
// JSON Formatter
// ============================================================================

/// JSON formatter.
pub struct JsonFormatter {
    pretty: bool,
}

impl JsonFormatter {
    /// Creates a new JSON formatter.
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }

    /// Formats any serializable value.
    pub fn format<T: Serialize>(&self, data: &T) -> Result<String> {
        let json = if self.pretty {
            serde_json::to_string_pretty(data)?
        } else {
            serde_json::to_string(data)?
        };
        Ok(json)
    }

    /// Formats usage results.
    pub fn format_results(
        &self,
        results: &HashMap<ProviderKind, Result<UsageSnapshot, String>>,
    ) -> Result<String> {
        let outputs: Vec<ProviderOutput> = results
            .iter()
            .map(|(provider, result)| self.snapshot_to_output(*provider, result))
            .collect();

        if outputs.len() == 1 {
            self.format(&outputs[0])
        } else {
            self.format(&outputs)
        }
    }

    /// Converts a snapshot result to output.
    fn snapshot_to_output(
        &self,
        provider: ProviderKind,
        result: &Result<UsageSnapshot, String>,
    ) -> ProviderOutput {
        let provider_name = format!("{:?}", provider).to_lowercase();

        match result {
            Ok(snapshot) => {
                let usage = UsageOutput {
                    primary: snapshot.primary.as_ref().map(|w| self.window_to_output(w)),
                    secondary: snapshot.secondary.as_ref().map(|w| self.window_to_output(w)),
                    tertiary: snapshot.tertiary.as_ref().map(|w| self.window_to_output(w)),
                    updated_at: snapshot.updated_at,
                    identity: snapshot.identity.as_ref().map(|id| IdentityOutput {
                        account_email: id.account_email.clone(),
                        account_organization: id.account_organization.clone(),
                        plan_name: id.plan_name.clone(),
                        login_method: id.login_method.as_ref().map(|m| format!("{:?}", m)),
                    }),
                };

                // Credits would come from separate store
                let credits: Option<CreditsOutput> = None;

                ProviderOutput {
                    provider: provider_name,
                    version: None, // TODO: get from provider descriptor
                    source: self.format_source(&snapshot.fetch_source),
                    status: None,
                    usage: Some(usage),
                    credits,
                    error: None,
                }
            }
            Err(e) => ProviderOutput {
                provider: provider_name,
                version: None,
                source: "unknown".to_string(),
                status: None,
                usage: None,
                credits: None,
                error: Some(e.clone()),
            },
        }
    }

    /// Converts a window to output.
    fn window_to_output(&self, window: &UsageWindow) -> WindowOutput {
        WindowOutput {
            used_percent: window.used_percent,
            window_minutes: window.window_minutes,
            resets_at: window.resets_at,
        }
    }

    /// Formats fetch source.
    fn format_source(&self, source: &FetchSource) -> String {
        match source {
            FetchSource::OAuth => "oauth".to_string(),
            FetchSource::CLI => "cli".to_string(),
            FetchSource::Web => "web".to_string(),
            FetchSource::LocalProbe => "local".to_string(),
            FetchSource::Api => "api".to_string(),
            FetchSource::Auto => "auto".to_string(),
        }
    }

    /// Formats cost results.
    pub fn format_cost_results(
        &self,
        results: &HashMap<ProviderKind, CostUsageSnapshot>,
    ) -> Result<String> {
        let outputs: Vec<CostOutput> = results
            .iter()
            .map(|(provider, cost)| CostOutput {
                provider: format!("{:?}", provider).to_lowercase(),
                total_tokens: cost.total_tokens,
                total_cost_usd: cost.total_cost_usd,
                daily: cost
                    .daily
                    .iter()
                    .map(|d| DailyCostOutput {
                        date: d.date.format("%Y-%m-%d").to_string(),
                        tokens: d.tokens,
                        cost_usd: d.cost_usd,
                    })
                    .collect(),
            })
            .collect();

        if outputs.len() == 1 {
            self.format(&outputs[0])
        } else {
            self.format(&outputs)
        }
    }

    /// Formats provider list.
    pub fn format_providers(&self, providers: &[ProviderDescriptor]) -> Result<String> {
        let outputs: Vec<ProviderInfoOutput> = providers
            .iter()
            .map(|desc| ProviderInfoOutput {
                id: format!("{:?}", desc.id).to_lowercase(),
                display_name: desc.display_name().to_string(),
                cli_name: desc.cli_name().to_string(),
                default_enabled: desc.metadata.default_enabled,
                is_primary: desc.metadata.is_primary_provider,
                supports_credits: desc.metadata.supports_credits,
                supports_opus: desc.metadata.supports_opus,
                dashboard_url: desc.metadata.dashboard_url.clone(),
                status_page_url: desc.metadata.status_page_url.clone(),
            })
            .collect();

        self.format(&outputs)
    }

    /// Formats summary.
    pub fn format_summary(
        &self,
        results: &HashMap<ProviderKind, Option<UsageSnapshot>>,
    ) -> Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SummaryItem {
            provider: String,
            status: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            primary_percent: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            secondary_percent: Option<f64>,
        }

        let items: Vec<SummaryItem> = results
            .iter()
            .map(|(provider, snapshot)| {
                let (status, primary, secondary) = match snapshot {
                    Some(snap) => (
                        "ok".to_string(),
                        snap.primary.as_ref().map(|w| w.used_percent),
                        snap.secondary.as_ref().map(|w| w.used_percent),
                    ),
                    None => ("error".to_string(), None, None),
                };

                SummaryItem {
                    provider: format!("{:?}", provider).to_lowercase(),
                    status,
                    primary_percent: primary,
                    secondary_percent: secondary,
                }
            })
            .collect();

        self.format(&items)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_pretty() {
        let formatter = JsonFormatter::new(true);
        let data = serde_json::json!({"key": "value"});
        let output = formatter.format(&data).unwrap();
        assert!(output.contains('\n'));
    }

    #[test]
    fn test_format_compact() {
        let formatter = JsonFormatter::new(false);
        let data = serde_json::json!({"key": "value"});
        let output = formatter.format(&data).unwrap();
        assert!(!output.contains('\n'));
    }

    #[test]
    fn test_window_output() {
        let formatter = JsonFormatter::new(false);
        let window = UsageWindow::new(50.0);
        let output = formatter.window_to_output(&window);
        assert_eq!(output.used_percent, 50.0);
    }

    #[test]
    fn test_source_format() {
        let formatter = JsonFormatter::new(false);
        assert_eq!(formatter.format_source(&FetchSource::OAuth), "oauth");
        assert_eq!(formatter.format_source(&FetchSource::CLI), "cli");
        assert_eq!(formatter.format_source(&FetchSource::Web), "web");
    }
}
