//! VertexAI response parser.

use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::debug;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct VertexAIUsageResponse {
    #[serde(default)]
    pub operations: Option<Vec<VertexAIOperation>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct VertexAIOperation {
    pub name: Option<String>,
    pub done: Option<bool>,
}

#[allow(dead_code)]
pub fn parse_vertexai_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing VertexAI response");

    let _response: VertexAIUsageResponse = serde_json::from_str(json_str)
        .map_err(|e| FetchError::InvalidResponse(format!("Invalid JSON: {}", e)))?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::OAuth;

    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let json = r#"{}"#;
        let snapshot = parse_vertexai_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }
}
