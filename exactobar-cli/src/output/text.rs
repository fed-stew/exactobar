//! Text output formatting with progress bars and colors.

use chrono::{DateTime, Duration, Local, Utc};
use exactobar_core::{FetchSource, ProviderKind, UsageSnapshot, UsageWindow};
use exactobar_providers::ProviderDescriptor;
use exactobar_store::CostUsageSnapshot;
use std::collections::HashMap;

// ============================================================================
// ANSI Colors
// ============================================================================

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";

// Progress bar characters
const BAR_FULL: char = '█';
const BAR_EMPTY: char = '░';

/// Text formatter with optional colors.
pub struct TextFormatter {
    use_colors: bool,
    show_reset_countdown: bool,
    bar_width: usize,
}

impl TextFormatter {
    /// Creates a new text formatter.
    pub fn new(use_colors: bool) -> Self {
        Self {
            use_colors,
            show_reset_countdown: true,
            bar_width: 10,
        }
    }

    /// Set the progress bar width.
    #[allow(dead_code)]
    pub fn with_bar_width(mut self, width: usize) -> Self {
        self.bar_width = width;
        self
    }

    /// Formats usage for a provider.
    pub fn format_usage(
        &self,
        snapshot: &UsageSnapshot,
        desc: Option<&ProviderDescriptor>,
_show_credits: bool,
    ) -> String {
        let mut lines = Vec::new();

        // Header: "Claude Code (oauth)"
        let name = desc.map(|d| d.display_name()).unwrap_or("Unknown");
        let source = self.format_source(&snapshot.fetch_source);

        lines.push(format!(
            "{} ({})",
            self.bold(name),
            source
        ));

        // Primary window (Session)
        if let Some(primary) = &snapshot.primary {
            let label = desc
                .map(|d| d.metadata.session_label.as_str())
                .unwrap_or("Session");
            lines.push(self.format_window(primary, label));
        }

        // Secondary window (Weekly)
        if let Some(secondary) = &snapshot.secondary {
            let label = desc
                .map(|d| d.metadata.weekly_label.as_str())
                .unwrap_or("Weekly");
            lines.push(self.format_window(secondary, label));
        }

        // Tertiary window (Opus)
        if let Some(tertiary) = &snapshot.tertiary {
            let label = desc
                .and_then(|d| d.metadata.opus_label.as_deref())
                .unwrap_or("Opus");
            lines.push(self.format_window(tertiary, label));
        }

        // Credits would come from separate store
        // For now we skip this as UsageSnapshot doesn't have credits directly

        // Identity
        if let Some(identity) = &snapshot.identity {
            if let Some(email) = &identity.account_email {
                lines.push(format!("Account: {}", self.cyan(email)));
            }
            if let Some(org) = &identity.account_organization {
                lines.push(format!("Org:     {}", org));
            }
            if let Some(plan) = &identity.plan_name {
                lines.push(format!("Plan:    {}", self.blue(plan)));
            }
        }

        lines.join("\n")
    }

    /// Formats a usage window with progress bar.
    fn format_window(&self, window: &UsageWindow, label: &str) -> String {
        let remaining = 100.0 - window.used_percent;
        let bar = self.progress_bar(remaining);
        let pct_str = self.color_for_percent(remaining, &format!("{:.0}% left", remaining));

        let mut result = format!("{:<8} {} {}", format!("{}:", label), bar, pct_str);

        // Add reset time
        if let Some(resets_at) = window.resets_at {
            let reset_str = self.format_reset_time(resets_at);
            result.push_str(&format!("\n         Resets {}", self.dim(&reset_str)));
        } else if let Some(desc) = &window.reset_description {
            result.push_str(&format!("\n         Resets {}", self.dim(desc)));
        }

        result
    }

    /// Formats a progress bar.
    pub fn progress_bar(&self, percent_remaining: f64) -> String {
        let filled = ((percent_remaining / 100.0) * self.bar_width as f64).round() as usize;
        let empty = self.bar_width.saturating_sub(filled);

        let bar = format!(
            "{}{}",
            BAR_FULL.to_string().repeat(filled),
            BAR_EMPTY.to_string().repeat(empty)
        );

        self.color_for_percent(percent_remaining, &bar)
    }

    /// Formats reset time as countdown or absolute.
    fn format_reset_time(&self, resets_at: DateTime<Utc>) -> String {
        let now = Utc::now();
        let local_reset = resets_at.with_timezone(&Local);

        if resets_at <= now {
            return "now".to_string();
        }

        let diff = resets_at - now;

        if self.show_reset_countdown && diff < Duration::hours(24) {
            // Show as relative time
            if diff < Duration::hours(1) {
                let mins = diff.num_minutes();
                format!("in {} minute{}", mins, if mins == 1 { "" } else { "s" })
            } else {
                let hours = diff.num_hours();
                let mins = diff.num_minutes() % 60;
                if mins > 0 {
                    format!("in {}h {}m", hours, mins)
                } else {
                    format!("in {} hour{}", hours, if hours == 1 { "" } else { "s" })
                }
            }
        } else {
            // Show as absolute time
            let today = Local::now().date_naive();
            let reset_date = local_reset.date_naive();

            if reset_date == today {
                format!("today at {}", local_reset.format("%l:%M %p").to_string().trim())
            } else if reset_date == today + chrono::Days::new(1) {
                format!("tomorrow at {}", local_reset.format("%l:%M %p").to_string().trim())
            } else {
                format!("{}", local_reset.format("%a at %l:%M %p").to_string().trim())
            }
        }
    }

    /// Formats fetch source for display.
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

    /// Formats cost usage.
    pub fn format_cost(
        &self,
        cost: &CostUsageSnapshot,
        desc: Option<&ProviderDescriptor>,
    ) -> String {
        let mut lines = Vec::new();

        let name = desc.map(|d| d.display_name()).unwrap_or("Unknown");
        lines.push(format!("{} Token Cost Report", self.bold(name)));
        lines.push("─".repeat(40));

        lines.push(format!(
            "Total tokens: {}",
            self.format_number(cost.total_tokens as f64)
        ));
        lines.push(format!(
            "Total cost:   {}",
            self.green(&format!("${:.2}", cost.total_cost_usd))
        ));

        if !cost.daily.is_empty() {
            lines.push(String::new());
            lines.push(self.dim("Daily breakdown:"));
            for day in &cost.daily {
                lines.push(format!(
                    "  {} - {} tokens (${:.2})",
                    day.date.format("%Y-%m-%d"),
                    self.format_number(day.tokens as f64),
                    day.cost_usd
                ));
            }
        }

        lines.join("\n")
    }

    /// Formats provider list header.
    pub fn format_providers_header(&self) -> String {
        format!(
            "{:<15} {:<10} {:<10} {:<8} {}",
            self.bold("Provider"),
            self.bold("CLI"),
            self.bold("Default"),
            self.bold("Primary"),
            self.bold("Dashboard")
        )
    }

    /// Formats a single provider line.
    pub fn format_provider_line(&self, desc: &ProviderDescriptor, installed: bool) -> String {
        let status = if installed {
            self.green("✓")
        } else {
            self.dim("−")
        };

        let default_str = if desc.metadata.default_enabled {
            self.green("✓")
        } else {
            self.dim("−")
        };

        let primary_str = if desc.metadata.is_primary_provider {
            self.green("✓")
        } else {
            self.dim("−")
        };

        let dashboard = desc
            .metadata
            .dashboard_url
            .as_deref()
            .unwrap_or("−");

        format!(
            "{:<15} {:<10} {:<10} {:<8} {}",
            format!("{} {}", desc.display_name(), status),
            desc.cli_name(),
            default_str,
            primary_str,
            dashboard
        )
    }

    /// Formats a summary of all providers.
    pub fn format_summary(
        &self,
        results: &HashMap<ProviderKind, Option<UsageSnapshot>>,
    ) -> String {
        let mut lines = Vec::new();

        lines.push(self.bold("ExactoBar Summary"));
        lines.push("─".repeat(50));
        lines.push(String::new());

        // Sort by provider kind for consistent order
        let mut sorted: Vec<_> = results.iter().collect();
        sorted.sort_by_key(|(k, _)| format!("{:?}", k));

        for (provider, snapshot) in sorted {
            let desc = exactobar_providers::ProviderRegistry::get(*provider);
            let name = desc.map(|d| d.display_name()).unwrap_or("Unknown");

            if let Some(snap) = snapshot {
                if let Some(primary) = &snap.primary {
                    let remaining = 100.0 - primary.used_percent;
                    let bar = self.progress_bar(remaining);
                    let pct = self.color_for_percent(remaining, &format!("{:.0}%", remaining));
                    lines.push(format!("{:<12} {} {}", name, bar, pct));
                } else {
                    lines.push(format!("{:<12} {}", name, self.dim("No data")));
                }
            } else {
                lines.push(format!("{:<12} {}", name, self.red("Error")));
            }
        }

        lines.join("\n")
    }

    /// Formats an error message.
    pub fn format_error(&self, provider: &str, error: &str) -> String {
        format!(
            "{}: {} - {}",
            self.bold(provider),
            self.red("Error"),
            error
        )
    }

    // ========================================================================
    // Color/style helpers
    // ========================================================================

    fn color_for_percent(&self, percent: f64, text: &str) -> String {
        if !self.use_colors {
            return text.to_string();
        }

        if percent < 20.0 {
            self.red(text)
        } else if percent < 50.0 {
            self.yellow(text)
        } else {
            self.green(text)
        }
    }

    fn format_number(&self, n: f64) -> String {
        if n >= 1_000_000.0 {
            format!("{:.1}M", n / 1_000_000.0)
        } else if n >= 1_000.0 {
            format!("{:.1}K", n / 1_000.0)
        } else {
            format!("{:.0}", n)
        }
    }

    fn bold(&self, text: &str) -> String {
        if self.use_colors {
            format!("{}{}{}", BOLD, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn dim(&self, text: &str) -> String {
        if self.use_colors {
            format!("{}{}{}", DIM, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn green(&self, text: &str) -> String {
        if self.use_colors {
            format!("{}{}{}", GREEN, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn yellow(&self, text: &str) -> String {
        if self.use_colors {
            format!("{}{}{}", YELLOW, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn red(&self, text: &str) -> String {
        if self.use_colors {
            format!("{}{}{}", RED, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn blue(&self, text: &str) -> String {
        if self.use_colors {
            format!("{}{}{}", BLUE, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn cyan(&self, text: &str) -> String {
        if self.use_colors {
            format!("{}{}{}", CYAN, text, RESET)
        } else {
            text.to_string()
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use exactobar_core::UsageWindow;

    #[test]
    fn test_progress_bar_full() {
        let formatter = TextFormatter::new(false);
        let bar = formatter.progress_bar(100.0);
        assert_eq!(bar, "██████████");
    }

    #[test]
    fn test_progress_bar_empty() {
        let formatter = TextFormatter::new(false);
        let bar = formatter.progress_bar(0.0);
        assert_eq!(bar, "░░░░░░░░░░");
    }

    #[test]
    fn test_progress_bar_half() {
        let formatter = TextFormatter::new(false);
        let bar = formatter.progress_bar(50.0);
        assert_eq!(bar, "█████░░░░░");
    }

    #[test]
    fn test_format_number() {
        let formatter = TextFormatter::new(false);
        assert_eq!(formatter.format_number(500.0), "500");
        assert_eq!(formatter.format_number(1500.0), "1.5K");
        assert_eq!(formatter.format_number(1500000.0), "1.5M");
    }

    #[test]
    fn test_color_for_percent() {
        let formatter = TextFormatter::new(true);
        let low = formatter.color_for_percent(15.0, "test");
        assert!(low.contains(RED));

        let mid = formatter.color_for_percent(35.0, "test");
        assert!(mid.contains(YELLOW));

        let high = formatter.color_for_percent(75.0, "test");
        assert!(high.contains(GREEN));
    }

    #[test]
    fn test_format_window() {
        let formatter = TextFormatter::new(false);
        let window = UsageWindow::new(28.0);
        let output = formatter.format_window(&window, "Session");
        assert!(output.contains("Session:"));
        assert!(output.contains("72% left"));
    }
}
