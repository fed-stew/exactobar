//! CLI output formatting tests.
//!
//! These tests verify that CLI output is correctly formatted for both
//! text and JSON output modes.

#[cfg(test)]
mod text_formatter_tests {
    use super::super::text::TextFormatter;
    use exactobar_core::{FetchSource, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow};
    use std::collections::HashMap;

    #[test]
    fn test_progress_bar_empty() {
        let formatter = TextFormatter::new(false);
        let bar = formatter.progress_bar(0.0);
        assert_eq!(bar, "░░░░░░░░░░");
    }

    #[test]
    fn test_progress_bar_full() {
        let formatter = TextFormatter::new(false);
        let bar = formatter.progress_bar(100.0);
        assert_eq!(bar, "██████████");
    }

    #[test]
    fn test_progress_bar_half() {
        let formatter = TextFormatter::new(false);
        let bar = formatter.progress_bar(50.0);
        assert_eq!(bar, "█████░░░░░");
    }

    #[test]
    fn test_progress_bar_boundary_values() {
        let formatter = TextFormatter::new(false);

        // Test various percentages
        let test_cases = vec![
            (0.0, "░░░░░░░░░░"),
            (10.0, "█░░░░░░░░░"),
            (25.0, "███░░░░░░░"), // 2.5 rounds to 3 blocks
            (50.0, "█████░░░░░"),
            (75.0, "████████░░"), // 7.5 rounds to 8 blocks
            (100.0, "██████████"),
        ];

        for (percent, expected) in test_cases {
            let bar = formatter.progress_bar(percent);
            assert_eq!(bar, expected, "Failed for {}%", percent);
        }
    }

    #[test]
    fn test_progress_bar_with_colors() {
        let formatter = TextFormatter::new(true);

        // Low remaining (critical) - should be red
        let bar = formatter.progress_bar(10.0);
        assert!(bar.contains("\x1b[31m"), "Should be red for <20%");

        // Medium remaining (warning) - should be yellow
        let bar = formatter.progress_bar(40.0);
        assert!(bar.contains("\x1b[33m"), "Should be yellow for <50%");

        // High remaining (good) - should be green
        let bar = formatter.progress_bar(80.0);
        assert!(bar.contains("\x1b[32m"), "Should be green for >=50%");
    }

    #[test]
    fn test_format_usage_with_primary_only() {
        let formatter = TextFormatter::new(false);

        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(50.0));
        snapshot.fetch_source = FetchSource::CLI;

        let output = formatter.format_usage(&snapshot, None, false);

        assert!(output.contains("Unknown")); // No descriptor
        assert!(output.contains("cli"));
        assert!(output.contains("50%"));
    }

    #[test]
    fn test_format_usage_with_all_windows() {
        let formatter = TextFormatter::new(false);

        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(25.0));
        snapshot.secondary = Some(UsageWindow::new(50.0));
        snapshot.tertiary = Some(UsageWindow::new(75.0));

        let output = formatter.format_usage(&snapshot, None, false);

        // Should contain all three windows
        assert!(output.contains("75%")); // 100 - 25 = 75% remaining for primary
        assert!(output.contains("50%")); // 100 - 50 = 50% remaining for secondary
        assert!(output.contains("25%")); // 100 - 75 = 25% remaining for tertiary
    }

    #[test]
    fn test_format_usage_with_identity() {
        let formatter = TextFormatter::new(false);

        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(50.0));

        let mut identity = ProviderIdentity::new(ProviderKind::Claude);
        identity.account_email = Some("user@example.com".to_string());
        identity.plan_name = Some("Pro".to_string());
        snapshot.identity = Some(identity);

        let output = formatter.format_usage(&snapshot, None, false);

        assert!(output.contains("user@example.com"));
        assert!(output.contains("Pro"));
    }

    #[test]
    fn test_format_summary_multiple_providers() {
        let formatter = TextFormatter::new(false);

        let mut results = HashMap::new();

        let mut snapshot1 = UsageSnapshot::new();
        snapshot1.primary = Some(UsageWindow::new(25.0));
        results.insert(ProviderKind::Claude, Some(snapshot1));

        let mut snapshot2 = UsageSnapshot::new();
        snapshot2.primary = Some(UsageWindow::new(75.0));
        results.insert(ProviderKind::Codex, Some(snapshot2));

        results.insert(ProviderKind::Cursor, None); // Error case

        let output = formatter.format_summary(&results);

        assert!(output.contains("ExactoBar Summary"));
        // Provider names should appear
        assert!(
            output.contains("Claude") || output.contains("Codex") || output.contains("Cursor")
        );
    }

    #[test]
    fn test_format_providers_header() {
        let formatter = TextFormatter::new(false);
        let header = formatter.format_providers_header();

        assert!(header.contains("Provider"));
        assert!(header.contains("CLI"));
        assert!(header.contains("Default"));
    }
}

#[cfg(test)]
mod json_formatter_tests {
    use super::super::json::JsonFormatter;
    use exactobar_core::{ProviderKind, UsageSnapshot, UsageWindow};
    use std::collections::HashMap;

    #[test]
    fn test_format_pretty_json() {
        let formatter = JsonFormatter::new(true);

        let data = serde_json::json!({"key": "value"});
        let output = formatter.format(&data).unwrap();

        // Pretty output should have newlines
        assert!(output.contains("\n"));
        assert!(output.contains("  ")); // Indentation
    }

    #[test]
    fn test_format_compact_json() {
        let formatter = JsonFormatter::new(false);

        let data = serde_json::json!({"key": "value"});
        let output = formatter.format(&data).unwrap();

        // Compact output should not have unnecessary whitespace
        assert_eq!(output, r#"{"key":"value"}"#);
    }

    #[test]
    fn test_format_results_success() {
        let formatter = JsonFormatter::new(true);

        let mut results = HashMap::new();
        let snapshot = UsageSnapshot::new();
        results.insert(ProviderKind::Claude, Ok(snapshot));

        let output = formatter.format_results(&results).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        // Single provider should be an object, not array
        assert!(parsed.get("provider").is_some() || parsed.get("usage").is_some());
    }

    #[test]
    fn test_format_results_error() {
        let formatter = JsonFormatter::new(true);

        let mut results = HashMap::new();
        results.insert(ProviderKind::Claude, Err("Connection timeout".to_string()));

        let output = formatter.format_results(&results).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("error").is_some());
    }

    #[test]
    fn test_format_summary_json() {
        let formatter = JsonFormatter::new(true);

        let mut results = HashMap::new();

        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(45.5));
        snapshot.secondary = Some(UsageWindow::new(20.0));
        results.insert(ProviderKind::Claude, Some(snapshot));

        results.insert(ProviderKind::Codex, None);

        let output = formatter.format_summary(&results).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_array());
    }

    #[test]
    fn test_format_empty_results() {
        let formatter = JsonFormatter::new(true);

        let results: HashMap<ProviderKind, Result<UsageSnapshot, String>> = HashMap::new();
        let output = formatter.format_results(&results).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_array());
        assert!(parsed.as_array().unwrap().is_empty());
    }
}

// ============================================================================
// Output Snapshot Tests (for regression testing)
// ============================================================================

#[cfg(test)]
mod output_snapshot_tests {
    use super::super::text::TextFormatter;
    use exactobar_core::{FetchSource, UsageSnapshot, UsageWindow};

    /// These tests capture expected output format for regression testing.
    /// If the output format changes, these tests will fail.

    #[test]
    fn test_progress_bar_width_consistency() {
        let formatter = TextFormatter::new(false);

        // All progress bars should have the same width
        for percent in [0.0, 25.0, 50.0, 75.0, 100.0] {
            let bar = formatter.progress_bar(percent);
            // Each character in the bar is a unicode block
            let char_count: usize = bar.chars().count();
            assert_eq!(char_count, 10, "Bar for {}% has {} chars", percent, char_count);
        }
    }

    #[test]
    fn test_output_contains_fetch_source() {
        let formatter = TextFormatter::new(false);

        let sources = vec![
            (FetchSource::CLI, "cli"),
            (FetchSource::OAuth, "oauth"),
            (FetchSource::Api, "api"),
            (FetchSource::Web, "web"),
            (FetchSource::LocalProbe, "local"),
        ];

        for (source, expected_label) in sources {
            let mut snapshot = UsageSnapshot::new();
            snapshot.fetch_source = source;
            snapshot.primary = Some(UsageWindow::new(50.0));

            let output = formatter.format_usage(&snapshot, None, false);
            assert!(
                output.contains(expected_label),
                "Output should contain '{}' for {:?}",
                expected_label,
                source
            );
        }
    }
}
