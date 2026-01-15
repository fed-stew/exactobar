//! Serde serialization/deserialization tests for core types.
//!
//! These tests verify that all core types can be correctly serialized to JSON
//! and deserialized back, preserving all data through the round-trip.

use chrono::{Duration, Utc};
use serde_json;

use crate::{
    Credits, CostUsageSnapshot, DailyUsageEntry, FetchSource, IconStyle, LoginMethod,
    ModelBreakdown, Provider, ProviderBranding, ProviderColor, ProviderIdentity, ProviderKind,
    ProviderMetadata, ProviderStatus, Quota, StatusIndicator, UsageData, UsageSnapshot,
    UsageWindow,
};

// ============================================================================
// ProviderKind Serde Tests
// ============================================================================

#[test]
fn test_provider_kind_serde_roundtrip_all_variants() {
    for kind in ProviderKind::all() {
        let json = serde_json::to_string(kind).unwrap();
        let deserialized: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(*kind, deserialized, "Round-trip failed for {:?}", kind);
    }
}

#[test]
fn test_provider_kind_deserialize_lowercase() {
    // ProviderKind uses serde(rename_all = "lowercase")
    let test_cases = vec![
        (r#""codex""#, ProviderKind::Codex),
        (r#""claude""#, ProviderKind::Claude),
        (r#""cursor""#, ProviderKind::Cursor),
        (r#""gemini""#, ProviderKind::Gemini),
        (r#""copilot""#, ProviderKind::Copilot),
        (r#""factory""#, ProviderKind::Factory),
        (r#""vertexai""#, ProviderKind::VertexAI),
        (r#""zai""#, ProviderKind::Zai),
        (r#""augment""#, ProviderKind::Augment),
        (r#""kiro""#, ProviderKind::Kiro),
        (r#""antigravity""#, ProviderKind::Antigravity),
        (r#""minimax""#, ProviderKind::MiniMax),
    ];

    for (json, expected) in test_cases {
        let result: ProviderKind = serde_json::from_str(json).unwrap();
        assert_eq!(result, expected, "Failed for {}", json);
    }
}

#[test]
fn test_provider_kind_invalid_deserialize() {
    let result: Result<ProviderKind, _> = serde_json::from_str(r#""invalid_provider""#);
    assert!(result.is_err());
}

// ============================================================================
// UsageSnapshot Serde Tests
// ============================================================================

#[test]
fn test_usage_snapshot_empty_roundtrip() {
    let snapshot = UsageSnapshot::new();
    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: UsageSnapshot = serde_json::from_str(&json).unwrap();
    
    assert!(deserialized.primary.is_none());
    assert!(deserialized.secondary.is_none());
    assert!(deserialized.tertiary.is_none());
}

#[test]
fn test_usage_snapshot_full_roundtrip() {
    let mut snapshot = UsageSnapshot::new();
    
    snapshot.primary = Some(UsageWindow {
        used_percent: 45.5,
        window_minutes: Some(300),
        resets_at: Some(Utc::now() + Duration::hours(2)),
        reset_description: Some("in 2 hours".to_string()),
    });
    
    snapshot.secondary = Some(UsageWindow::new(20.0));
    snapshot.tertiary = Some(UsageWindow::new(75.0));
    snapshot.fetch_source = FetchSource::CLI;
    
    let mut identity = ProviderIdentity::new(ProviderKind::Claude);
    identity.account_email = Some("test@example.com".to_string());
    identity.plan_name = Some("Pro".to_string());
    snapshot.identity = Some(identity);
    
    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: UsageSnapshot = serde_json::from_str(&json).unwrap();
    
    assert!(deserialized.primary.is_some());
    assert_eq!(deserialized.primary.as_ref().unwrap().used_percent, 45.5);
    assert_eq!(deserialized.primary.as_ref().unwrap().window_minutes, Some(300));
    assert!(deserialized.secondary.is_some());
    assert!(deserialized.tertiary.is_some());
    assert!(deserialized.identity.is_some());
    assert_eq!(deserialized.identity.as_ref().unwrap().account_email, Some("test@example.com".to_string()));
}

// ============================================================================
// UsageWindow Serde Tests
// ============================================================================

#[test]
fn test_usage_window_boundary_values() {
    let test_cases = vec![
        0.0_f64,    // Minimum
        50.0,       // Mid-point
        100.0,      // Maximum
        0.001,      // Very small
        99.999,     // Near maximum
    ];
    
    for percent in test_cases {
        let window = UsageWindow::new(percent);
        let json = serde_json::to_string(&window).unwrap();
        let deserialized: UsageWindow = serde_json::from_str(&json).unwrap();
        assert!((deserialized.used_percent - percent).abs() < 0.0001, 
            "Failed for {}", percent);
    }
}

#[test]
fn test_usage_window_with_reset_time() {
    let mut window = UsageWindow::new(50.0);
    let future_time = Utc::now() + Duration::hours(5);
    window.resets_at = Some(future_time);
    window.reset_description = Some("in 5 hours".to_string());
    
    let json = serde_json::to_string(&window).unwrap();
    let deserialized: UsageWindow = serde_json::from_str(&json).unwrap();
    
    assert!(deserialized.resets_at.is_some());
    assert_eq!(deserialized.reset_description, Some("in 5 hours".to_string()));
}

// ============================================================================
// Credits Serde Tests
// ============================================================================

#[test]
fn test_credits_roundtrip() {
    let mut credits = Credits::new(25.50);
    credits.total = Some(100.0);
    
    let json = serde_json::to_string(&credits).unwrap();
    let deserialized: Credits = serde_json::from_str(&json).unwrap();
    
    assert!((deserialized.remaining - 25.50).abs() < 0.001);
    assert_eq!(deserialized.total, Some(100.0));
}

#[test]
fn test_credits_zero_total() {
    let mut credits = Credits::new(0.0);
    credits.total = Some(0.0);
    
    let json = serde_json::to_string(&credits).unwrap();
    let deserialized: Credits = serde_json::from_str(&json).unwrap();
    
    // Should not panic on percentage calculation
    assert_eq!(deserialized.remaining_percent(), Some(100.0));
}

// ============================================================================
// ProviderIdentity Serde Tests
// ============================================================================

#[test]
fn test_provider_identity_full_roundtrip() {
    let mut identity = ProviderIdentity::new(ProviderKind::Codex);
    identity.account_email = Some("user@company.com".to_string());
    identity.account_organization = Some("Acme Corp".to_string());
    identity.plan_name = Some("Enterprise".to_string());
    identity.login_method = Some(LoginMethod::OAuth);
    
    let json = serde_json::to_string(&identity).unwrap();
    let deserialized: ProviderIdentity = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.provider_id, ProviderKind::Codex);
    assert_eq!(deserialized.account_email, Some("user@company.com".to_string()));
    assert_eq!(deserialized.account_organization, Some("Acme Corp".to_string()));
    assert_eq!(deserialized.login_method, Some(LoginMethod::OAuth));
}

// ============================================================================
// LoginMethod Serde Tests
// ============================================================================

#[test]
fn test_login_method_all_variants() {
    let variants = vec![
        LoginMethod::OAuth,
        LoginMethod::ApiKey,
        LoginMethod::BrowserCookies,
        LoginMethod::CLI,
        LoginMethod::DeviceFlow,
    ];
    
    for method in variants {
        let json = serde_json::to_string(&method).unwrap();
        let deserialized: LoginMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(method, deserialized);
    }
}

// ============================================================================
// StatusIndicator Serde Tests
// ============================================================================

#[test]
fn test_status_indicator_all_variants() {
    for indicator in StatusIndicator::all() {
        let json = serde_json::to_string(indicator).unwrap();
        let deserialized: StatusIndicator = serde_json::from_str(&json).unwrap();
        assert_eq!(*indicator, deserialized);
    }
}

#[test]
fn test_status_indicator_snake_case() {
    // Verify snake_case serialization
    let json = serde_json::to_string(&StatusIndicator::None).unwrap();
    assert_eq!(json, r#""none""#);
}

// ============================================================================
// FetchSource Serde Tests
// ============================================================================

#[test]
fn test_fetch_source_all_variants() {
    for source in FetchSource::all() {
        let json = serde_json::to_string(source).unwrap();
        let deserialized: FetchSource = serde_json::from_str(&json).unwrap();
        assert_eq!(*source, deserialized);
    }
}

// ============================================================================
// ProviderStatus Serde Tests
// ============================================================================

#[test]
fn test_provider_status_roundtrip() {
    let status = ProviderStatus::new(StatusIndicator::Minor, "Experiencing delays");
    
    let json = serde_json::to_string(&status).unwrap();
    let deserialized: ProviderStatus = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.indicator, StatusIndicator::Minor);
    assert_eq!(deserialized.description, "Experiencing delays");
}

// ============================================================================
// CostUsageSnapshot Serde Tests
// ============================================================================

#[test]
fn test_cost_usage_snapshot_with_daily_entries() {
    let mut snapshot = CostUsageSnapshot::new();
    snapshot.session_tokens = Some(5000);
    snapshot.session_cost_usd = Some(0.15);
    snapshot.last_30_days_tokens = Some(100000);
    snapshot.last_30_days_cost_usd = Some(3.50);
    
    let mut entry = DailyUsageEntry::new("2024-01-15");
    entry.input_tokens = Some(1000);
    entry.output_tokens = Some(500);
    entry.cost_usd = Some(0.05);
    snapshot.daily.push(entry);
    
    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: CostUsageSnapshot = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.session_tokens, Some(5000));
    assert_eq!(deserialized.daily.len(), 1);
    assert_eq!(deserialized.daily[0].date, "2024-01-15");
}

// ============================================================================
// ModelBreakdown Serde Tests
// ============================================================================

#[test]
fn test_model_breakdown_roundtrip() {
    let mut breakdown = ModelBreakdown::new("claude-3-opus-20240229");
    breakdown.cost_usd = Some(0.75);
    breakdown.input_tokens = Some(10000);
    breakdown.output_tokens = Some(5000);
    
    let json = serde_json::to_string(&breakdown).unwrap();
    let deserialized: ModelBreakdown = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.model_name, "claude-3-opus-20240229");
    assert_eq!(deserialized.total_tokens(), 15000);
}

// ============================================================================
// Provider Serde Tests
// ============================================================================

#[test]
fn test_provider_config_roundtrip() {
    let mut provider = Provider::new(ProviderKind::Claude);
    provider.enabled = true;
    provider.display_name = Some("My Claude".to_string());
    provider.api_key_env = Some("CLAUDE_API_KEY".to_string());
    
    let json = serde_json::to_string(&provider).unwrap();
    let deserialized: Provider = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.kind, ProviderKind::Claude);
    assert_eq!(deserialized.display_name, Some("My Claude".to_string()));
}

#[test]
fn test_provider_api_key_not_serialized() {
    let mut provider = Provider::new(ProviderKind::Codex);
    provider.api_key = Some("secret-key".to_string());
    
    let json = serde_json::to_string(&provider).unwrap();
    
    // api_key should be skipped in serialization
    assert!(!json.contains("secret-key"));
}

// ============================================================================
// ProviderColor Serde Tests
// ============================================================================

#[test]
fn test_provider_color_boundary_values() {
    let test_cases = vec![
        ProviderColor::new(0.0, 0.0, 0.0),      // Black
        ProviderColor::new(1.0, 1.0, 1.0),      // White
        ProviderColor::new(1.0, 0.0, 0.0),      // Red
        ProviderColor::new(0.5, 0.5, 0.5),      // Gray
    ];
    
    for color in test_cases {
        let json = serde_json::to_string(&color).unwrap();
        let deserialized: ProviderColor = serde_json::from_str(&json).unwrap();
        assert!((deserialized.red - color.red).abs() < 0.001);
        assert!((deserialized.green - color.green).abs() < 0.001);
        assert!((deserialized.blue - color.blue).abs() < 0.001);
    }
}

// ============================================================================
// IconStyle Serde Tests
// ============================================================================

#[test]
fn test_icon_style_all_variants() {
    let variants = vec![
        IconStyle::Codex,
        IconStyle::Claude,
        IconStyle::Cursor,
        IconStyle::Gemini,
        IconStyle::Copilot,
        IconStyle::Factory,
        IconStyle::VertexAI,
        IconStyle::Zai,
        IconStyle::Augment,
        IconStyle::Kiro,
        IconStyle::Antigravity,
        IconStyle::MiniMax,
        IconStyle::Combined,
    ];
    
    for style in variants {
        let json = serde_json::to_string(&style).unwrap();
        let deserialized: IconStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(style, deserialized);
    }
}

// ============================================================================
// Complex Nested Structures
// ============================================================================

#[test]
fn test_full_provider_metadata_roundtrip() {
    let metadata = ProviderMetadata::for_provider(ProviderKind::Claude);
    
    let json = serde_json::to_string(&metadata).unwrap();
    let deserialized: ProviderMetadata = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.id, ProviderKind::Claude);
    assert_eq!(deserialized.display_name, "Claude");
}

#[test]
fn test_provider_branding_roundtrip() {
    let branding = ProviderBranding::for_provider(ProviderKind::Codex);
    
    let json = serde_json::to_string(&branding).unwrap();
    let deserialized: ProviderBranding = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.icon_style, IconStyle::Codex);
}

// ============================================================================
// Quota Serde Tests
// ============================================================================

#[test]
fn test_quota_roundtrip() {
    let quota = Quota {
        provider_kind: ProviderKind::Gemini,
        total: 100000.0,
        used: 45000.0,
        remaining: 55000.0,
        unit: "tokens".to_string(),
        resets_at: Some(Utc::now() + Duration::days(30)),
    };
    
    let json = serde_json::to_string(&quota).unwrap();
    let deserialized: Quota = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.provider_kind, ProviderKind::Gemini);
    assert!((deserialized.usage_percentage() - 45.0).abs() < 0.001);
}

// ============================================================================
// UsageData Serde Tests
// ============================================================================

#[test]
fn test_usage_data_roundtrip() {
    let usage = UsageData {
        provider_kind: ProviderKind::Copilot,
        fetched_at: Utc::now(),
        current_usage: 750.0,
        limit: Some(1000.0),
        unit: "requests".to_string(),
        period_start: Some(Utc::now() - Duration::days(7)),
        period_end: Some(Utc::now() + Duration::days(23)),
        metadata: serde_json::json!({"extra": "data"}),
    };
    
    let json = serde_json::to_string(&usage).unwrap();
    let deserialized: UsageData = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.provider_kind, ProviderKind::Copilot);
    assert_eq!(deserialized.usage_percentage(), Some(75.0));
}

// ============================================================================
// Backward Compatibility Tests
// ============================================================================

#[test]
fn test_deserialize_minimal_usage_snapshot() {
    // Test that we can deserialize a minimal JSON without optional fields
    let json = r#"{
        "updated_at": "2024-01-15T10:00:00Z"
    }"#;
    
    let snapshot: UsageSnapshot = serde_json::from_str(json).unwrap();
    assert!(snapshot.primary.is_none());
    assert!(snapshot.identity.is_none());
}

#[test]
fn test_deserialize_with_unknown_fields() {
    // Test that unknown fields are ignored (forward compatibility)
    let json = r#"{
        "used_percent": 50.0,
        "unknown_field": "should be ignored"
    }"#;
    
    // UsageWindow should ignore unknown fields by default
    let result: Result<UsageWindow, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}
