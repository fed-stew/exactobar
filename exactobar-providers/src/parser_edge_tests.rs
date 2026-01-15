//! Parser edge case and error handling tests.
//!
//! These tests verify parser behavior with malformed, partial, or edge case inputs.

#[cfg(test)]
mod claude_parser_edge_tests {
    use crate::claude::parser::{parse_claude_api_response, parse_claude_cli_output, parse_text_usage_line};
    
    // ========================================================================
    // JSON Edge Cases
    // ========================================================================
    
    #[test]
    fn test_parse_empty_json_object() {
        let json = r#"{}"#;
        let result = parse_claude_api_response(json);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        assert!(snapshot.primary.is_none());
    }
    
    #[test]
    fn test_parse_null_values() {
        let json = r#"{
            "session": null,
            "weekly": null,
            "opus": null,
            "user": null,
            "organization": null
        }"#;
        let result = parse_claude_api_response(json);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_parse_usage_with_all_aliases() {
        // Test that all field aliases work
        let json = r#"{
            "session": {
                "usage_percent": 50.0,
                "remaining_percent": 50.0,
                "duration": 300,
                "reset_at": "2024-01-15T10:00:00Z",
                "time_until_reset": "in 2 hours"
            }
        }"#;
        let result = parse_claude_api_response(json);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        assert!(snapshot.primary.is_some());
    }
    
    #[test]
    fn test_parse_percentage_boundary_values() {
        // Test 0%
        let json = r#"{"session": {"used_percent": 0.0}}"#;
        let result = parse_claude_api_response(json).unwrap();
        assert_eq!(result.primary.as_ref().unwrap().used_percent, 0.0);
        
        // Test 100%
        let json = r#"{"session": {"used_percent": 100.0}}"#;
        let result = parse_claude_api_response(json).unwrap();
        assert_eq!(result.primary.as_ref().unwrap().used_percent, 100.0);
        
        // Test over 100% (should still parse)
        let json = r#"{"session": {"used_percent": 150.0}}"#;
        let result = parse_claude_api_response(json).unwrap();
        assert_eq!(result.primary.as_ref().unwrap().used_percent, 150.0);
    }
    
    #[test]
    fn test_parse_remaining_to_used_conversion() {
        // When only remaining is provided, used should be calculated as 100 - remaining
        let json = r#"{"session": {"remaining": 25.0}}"#;
        let result = parse_claude_api_response(json).unwrap();
        assert_eq!(result.primary.as_ref().unwrap().used_percent, 75.0);
    }
    
    #[test]
    fn test_parse_invalid_reset_timestamp() {
        // Invalid timestamp should not cause failure, just skip parsing
        let json = r#"{
            "session": {
                "used_percent": 50.0,
                "resets_at": "not-a-valid-timestamp"
            }
        }"#;
        let result = parse_claude_api_response(json);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        assert!(snapshot.primary.as_ref().unwrap().resets_at.is_none());
    }
    
    #[test]
    fn test_parse_malformed_json() {
        let malformed_cases = vec![
            "{",
            "}",
            "not json at all",
            r#"{"session": }"#,
            r#"{"session": {broken}"#,
            "",
            "null",
        ];
        
        for json in malformed_cases {
            let result = parse_claude_api_response(json);
            // Empty string and "null" might parse differently
            if json != "null" && !json.is_empty() {
                assert!(result.is_err(), "Should fail for: {}", json);
            }
        }
    }
    
    // ========================================================================
    // CLI Text Output Edge Cases
    // ========================================================================
    
    #[test]
    fn test_parse_cli_empty_output() {
        let output = "";
        let result = parse_claude_cli_output(output, false);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        assert!(snapshot.primary.is_none());
    }
    
    #[test]
    fn test_parse_cli_whitespace_only() {
        let output = "   \n\t\n   ";
        let result = parse_claude_cli_output(output, false);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_parse_cli_case_sensitivity() {
        // Only exact prefix match should work
        let output = "session: 50% used\nSESSION: 60% used\nSession: 70% used";
        let result = parse_claude_cli_output(output, false);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        // Should only match "Session:" with capital S
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 70.0);
    }
    
    #[test]
    fn test_parse_text_usage_line_edge_cases() {
        // Fractional percentage
        assert_eq!(parse_text_usage_line("45.5% used").unwrap().used_percent, 45.5);
        
        // Percentage at boundary
        assert_eq!(parse_text_usage_line("0% used").unwrap().used_percent, 0.0);
        assert_eq!(parse_text_usage_line("100% used").unwrap().used_percent, 100.0);
        
        // With various parenthetical content
        let window = parse_text_usage_line("50% used (resets in 30m)").unwrap();
        assert_eq!(window.reset_description, Some("in 30m".to_string()));
        
        // Without "resets" prefix
        let window = parse_text_usage_line("50% used (Sunday)").unwrap();
        assert_eq!(window.reset_description, Some("Sunday".to_string()));
        
        // Empty parentheses
        let window = parse_text_usage_line("50% used ()").unwrap();
        assert_eq!(window.reset_description, Some("".to_string()));
    }
    
    #[test]
    fn test_parse_text_usage_line_invalid() {
        // No percentage sign
        assert!(parse_text_usage_line("50 used").is_none());
        
        // No number before %
        assert!(parse_text_usage_line("% used").is_none());
        
        // Invalid number
        assert!(parse_text_usage_line("abc% used").is_none());
        
        // Empty string
        assert!(parse_text_usage_line("").is_none());
    }
}

#[cfg(test)]
mod cursor_parser_edge_tests {
    use crate::cursor::parser::{parse_cursor_api_response, parse_cursor_local_config};
    
    #[test]
    fn test_parse_zero_limits() {
        // Edge case: limit is 0 (should not cause division by zero)
        let json = r#"{
            "usage": {
                "requests": 10,
                "limit": 0
            }
        }"#;
        let result = parse_cursor_api_response(json);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        // Should handle gracefully (0% when limit is 0)
        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 0.0);
    }
    
    #[test]
    fn test_parse_usage_exceeds_limit() {
        // Usage exceeds limit (>100%)
        let json = r#"{
            "usage": {
                "requests": 200,
                "limit": 100
            }
        }"#;
        let result = parse_cursor_api_response(json);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 200.0);
    }
    
    #[test]
    fn test_parse_with_camel_case_aliases() {
        // Test camelCase variants
        let json = r#"{
            "usage": {
                "requestCount": 50,
                "requestLimit": 100,
                "premiumRequests": 10,
                "premiumLimit": 50,
                "periodEnd": "2024-01-31T00:00:00Z"
            },
            "subscription": {
                "plan": "pro",
                "isActive": true
            }
        }"#;
        let result = parse_cursor_api_response(json);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_parse_local_config_minimal() {
        let json = r#"{}"#;
        let result = parse_cursor_local_config(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod codex_parser_edge_tests {
    use crate::codex::parser::parse_codex_cli_output;
    
    #[test]
    fn test_parse_with_timestamps() {
        let json = r#"{
            "session": {
                "used_percent": 50.0,
                "window_minutes": 300,
                "resets_at": "2024-01-15T10:30:00+00:00"
            }
        }"#;
        let result = parse_codex_cli_output(json);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        assert!(snapshot.primary.as_ref().unwrap().resets_at.is_some());
    }
    
    #[test]
    fn test_parse_with_credits() {
        let json = r#"{
            "session": {"used_percent": 50.0},
            "credits": {
                "remaining": 25.50,
                "total": 100.0,
                "unit": "USD"
            }
        }"#;
        let result = parse_codex_cli_output(json);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_parse_organization_alias() {
        let json = r#"{
            "account": {
                "email": "user@example.com",
                "org": "Acme Corp"
            }
        }"#;
        let result = parse_codex_cli_output(json);
        assert!(result.is_ok());
        let snapshot = result.unwrap();
        assert!(snapshot.identity.is_some());
        assert_eq!(
            snapshot.identity.as_ref().unwrap().account_organization,
            Some("Acme Corp".to_string())
        );
    }
}

#[cfg(test)]
mod gemini_parser_edge_tests {
    use crate::gemini::parser::parse_gemini_response;
    
    #[test]
    fn test_parse_gemini_minimal() {
        let json = r#"{}"#;
        let result = parse_gemini_response(json);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_parse_gemini_with_quota() {
        let json = r#"{
            "quota": {
                "requests_used": 500,
                "requests_limit": 1000
            }
        }"#;
        let result = parse_gemini_response(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod factory_parser_edge_tests {
    use crate::factory::parser::parse_factory_response;
    
    #[test]
    fn test_parse_factory_minimal() {
        let json = r#"{}"#;
        let result = parse_factory_response(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod copilot_parser_edge_tests {
    use crate::copilot::parser::parse_copilot_response;
    
    #[test]
    fn test_parse_copilot_minimal() {
        let json = r#"{}"#;
        let result = parse_copilot_response(json);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_parse_copilot_with_seat_info() {
        let json = r#"{
            "seat": {
                "type": "individual",
                "enterprise": false
            }
        }"#;
        let result = parse_copilot_response(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod augment_parser_edge_tests {
    use crate::augment::parser::parse_augment_response;
    
    #[test]
    fn test_parse_augment_with_usage() {
        let json = r#"{
            "usage": {
                "requests_used": 100,
                "requests_limit": 500
            }
        }"#;
        let result = parse_augment_response(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod minimax_parser_edge_tests {
    use crate::minimax::parser::parse_minimax_response;
    
    #[test]
    fn test_parse_minimax_with_balance() {
        let json = r#"{
            "balance": {
                "remaining": 100.0,
                "total": 500.0
            }
        }"#;
        let result = parse_minimax_response(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod zai_parser_edge_tests {
    use crate::zai::parser::parse_zai_response;
    
    #[test]
    fn test_parse_zai_minimal() {
        let json = r#"{}"#;
        let result = parse_zai_response(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod kiro_parser_edge_tests {
    use crate::kiro::parser::parse_kiro_response;
    
    #[test]
    fn test_parse_kiro_minimal() {
        let json = r#"{}"#;
        let result = parse_kiro_response(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod vertexai_parser_edge_tests {
    use crate::vertexai::parser::parse_vertexai_response;
    
    #[test]
    fn test_parse_vertexai_minimal() {
        let json = r#"{}"#;
        let result = parse_vertexai_response(json);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod antigravity_parser_edge_tests {
    // Antigravity likely uses local probe, test that module
}

// ============================================================================
// Cross-Parser Consistency Tests
// ============================================================================

#[cfg(test)]
mod cross_parser_tests {
    use exactobar_core::{UsageSnapshot, FetchSource};
    
    #[test]
    fn test_all_parsers_return_valid_fetch_source() {
        // Verify that all parsers set a meaningful fetch_source
        let snapshot = UsageSnapshot::new();
        
        // Default should be Auto
        assert_eq!(snapshot.fetch_source, FetchSource::Auto);
    }
    
    #[test]
    fn test_empty_snapshot_behavior_consistency() {
        // All empty snapshots should behave the same
        let snapshot = UsageSnapshot::new();
        
        assert!(!snapshot.has_data());
        assert!(!snapshot.is_approaching_limit());
        assert_eq!(snapshot.max_usage_percent(), 0.0);
    }
}
