//! VertexAI OAuth credentials.
//!
//! This module handles OAuth credential loading and token refresh for VertexAI.
//! It reads credentials from Google Cloud SDK's Application Default Credentials (ADC)
//! and refreshes access tokens via Google's OAuth2 endpoint.
//!
//! ## Credential Sources
//!
//! 1. **Environment Variable Override**
//!    - `GOOGLE_APPLICATION_CREDENTIALS` - path to custom credentials file
//!
//! 2. **Application Default Credentials (ADC)**
//!    - `~/.config/gcloud/application_default_credentials.json`
//!    - Contains refresh token for offline access

use std::path::PathBuf;

use serde::Deserialize;
use tracing::{debug, info, instrument};

use super::error::VertexAIError;

// ============================================================================
// Constants
// ============================================================================

/// Google OAuth2 token endpoint.
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// HTTP client timeout for token refresh.
const HTTP_TIMEOUT_SECS: u64 = 10;

// ============================================================================
// Credential Paths
// ============================================================================

/// Get all possible credential paths, in priority order.
fn credential_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Environment variable override (highest priority)
    if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        paths.push(PathBuf::from(path));
    }

    // Application default credentials (standard location)
    if let Some(config) = dirs::config_dir() {
        paths.push(
            config
                .join("gcloud")
                .join("application_default_credentials.json"),
        );
    }

    paths
}

// ============================================================================
// Credentials
// ============================================================================

/// VertexAI OAuth credentials loaded from ADC file.
#[derive(Debug, Deserialize)]
pub struct VertexAICredentials {
    /// OAuth client ID.
    pub client_id: Option<String>,

    /// OAuth client secret.
    pub client_secret: Option<String>,

    /// Refresh token for obtaining new access tokens.
    pub refresh_token: Option<String>,

    /// Credential type (usually "authorized_user").
    #[serde(rename = "type")]
    pub credential_type: Option<String>,

    /// Quota project ID for billing.
    pub quota_project_id: Option<String>,
}

impl VertexAICredentials {
    /// Load credentials from the first available credential file.
    #[instrument]
    pub fn load() -> Result<Self, VertexAIError> {
        for path in credential_paths() {
            if path.exists() {
                debug!(path = %path.display(), "Found credentials file");

                let content =
                    std::fs::read_to_string(&path).map_err(|_| VertexAIError::NotLoggedIn)?;

                return serde_json::from_str(&content)
                    .map_err(|e| VertexAIError::ParseError(e.to_string()));
            }
        }

        Err(VertexAIError::NotLoggedIn)
    }

    /// Check if credentials have all required OAuth fields.
    pub fn has_oauth(&self) -> bool {
        self.client_id.is_some()
            && self.client_secret.is_some()
            && self.refresh_token.is_some()
    }

    /// Get the project ID if available.
    pub fn project_id(&self) -> Option<&str> {
        self.quota_project_id.as_deref()
    }

    /// Get the credential type.
    pub fn cred_type(&self) -> Option<&str> {
        self.credential_type.as_deref()
    }
}

// ============================================================================
// Token Refresher
// ============================================================================

/// OAuth token response from Google.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    expires_in: Option<u64>,
    #[allow(dead_code)]
    token_type: Option<String>,
}

/// OAuth token refresher for VertexAI.
///
/// Handles refreshing access tokens using the refresh token from ADC credentials.
pub struct VertexAITokenRefresher {
    http: reqwest::Client,
}

impl VertexAITokenRefresher {
    /// Create a new token refresher.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");

        Self { http }
    }

    /// Refresh an access token using the credentials' refresh token.
    #[instrument(skip(self, creds))]
    pub async fn refresh(&self, creds: &VertexAICredentials) -> Result<String, VertexAIError> {
        let client_id = creds
            .client_id
            .as_ref()
            .ok_or(VertexAIError::NotLoggedIn)?;

        let client_secret = creds
            .client_secret
            .as_ref()
            .ok_or(VertexAIError::NotLoggedIn)?;

        let refresh_token = creds
            .refresh_token
            .as_ref()
            .ok_or(VertexAIError::NotLoggedIn)?;

        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
        ];

        info!("Refreshing VertexAI OAuth token");

        let response = self
            .http
            .post(TOKEN_ENDPOINT)
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    VertexAIError::Timeout
                } else {
                    VertexAIError::ApiError(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(VertexAIError::ApiError(format!(
                "Token refresh failed: {} - {}",
                status, body
            )));
        }

        let token: TokenResponse = response
            .json()
            .await
            .map_err(|e| VertexAIError::ParseError(e.to_string()))?;

        debug!("Successfully refreshed OAuth token");
        Ok(token.access_token)
    }
}

impl Default for VertexAITokenRefresher {
    fn default() -> Self {
        Self::new()
    }
}


// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_paths_includes_adc() {
        let paths = credential_paths();
        assert!(
            paths.iter().any(|p| p
                .to_string_lossy()
                .contains("application_default_credentials.json")),
            "Should include ADC path"
        );
    }

    #[test]
    fn test_has_oauth_with_all_fields() {
        let creds = VertexAICredentials {
            client_id: Some("id".to_string()),
            client_secret: Some("secret".to_string()),
            refresh_token: Some("token".to_string()),
            credential_type: Some("authorized_user".to_string()),
            quota_project_id: Some("project".to_string()),
        };
        assert!(creds.has_oauth());
    }

    #[test]
    fn test_has_oauth_missing_fields() {
        let creds = VertexAICredentials {
            client_id: Some("id".to_string()),
            client_secret: None,
            refresh_token: Some("token".to_string()),
            credential_type: None,
            quota_project_id: None,
        };
        assert!(!creds.has_oauth());
    }

    #[test]
    fn test_parse_credentials() {
        let json = r#"{
            "client_id": "123.apps.googleusercontent.com",
            "client_secret": "secret",
            "refresh_token": "1//refresh",
            "type": "authorized_user",
            "quota_project_id": "my-project"
        }"#;

        let creds: VertexAICredentials = serde_json::from_str(json).unwrap();
        assert_eq!(creds.client_id, Some("123.apps.googleusercontent.com".to_string()));
        assert_eq!(creds.credential_type, Some("authorized_user".to_string()));
        assert_eq!(creds.project_id(), Some("my-project"));
        assert!(creds.has_oauth());
    }

    #[test]
    fn test_token_refresher_creation() {
        let refresher = VertexAITokenRefresher::new();
        // Just verify it doesn't panic
        drop(refresher);
    }

}
