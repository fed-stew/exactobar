//! Gemini response parser.

use exactobar_core::{FetchSource, UsageSnapshot, UsageWindow};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct GeminiUsageResponse {
    #[serde(default)]
    pub requests: Option<GeminiRequests>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiRequests {
    pub used: Option<u64>,
    pub limit: Option<u64>,
    #[allow(dead_code)]
    pub reset_at: Option<String>,
}

pub fn parse_gemini_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing Gemini response");

    let response: GeminiUsageResponse = serde_json::from_str(json_str)
        .map_err(|e| FetchError::InvalidResponse(format!("Invalid JSON: {}", e)))?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::CLI;

    if let Some(requests) = response.requests {
        if let (Some(used), Some(limit)) = (requests.used, requests.limit) {
            let percent = if limit > 0 {
                (used as f64 / limit as f64) * 100.0
            } else {
                0.0
            };
            snapshot.primary = Some(UsageWindow::new(percent));
        }
    }

    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemini() {
        let json = r#"{"requests": {"used": 50, "limit": 100}}"#;
        let snapshot = parse_gemini_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
    }

    #[test]
    fn test_parse_empty() {
        let json = r#"{}"#;
        let snapshot = parse_gemini_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }
}
