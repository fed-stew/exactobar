//! Integration tests for core snapshot types.

use exactobar_core::{ProviderKind, UsageSnapshot, UsageWindow};

#[test]
fn test_snapshot_serialization_roundtrip() {
    let snapshot = UsageSnapshot::new();
    let json = serde_json::to_string(&snapshot).unwrap();
    let parsed: UsageSnapshot = serde_json::from_str(&json).unwrap();
    assert!(!parsed.has_data());
}

#[test]
fn test_usage_window_validation() {
    let mut window = UsageWindow::new(50.0);
    assert!(window.validate().is_ok());
    
    window.used_percent = -10.0;
    assert!(window.validate().is_err());
}
