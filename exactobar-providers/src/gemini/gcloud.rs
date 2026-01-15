//! gcloud credential reader.
//!
//! This module reads OAuth credentials from Google Cloud SDK configuration.
//!
//! ## Credential Sources
//!
//! 1. **Application Default Credentials (ADC)**
//!    - `~/.config/gcloud/application_default_credentials.json`
//!    - Contains refresh token for offline access
//!
//! 2. **gcloud credentials.db**
//!    - `~/.config/gcloud/credentials.db` (SQLite)
//!    - Contains cached access tokens
//!
//! 3. **gcloud CLI**
//!    - `gcloud auth print-access-token`
//!    - Real-time token generation
//!
//! ## Example
//!
//! ```ignore
//! let creds = GcloudCredentials::load()?;
//! let token = creds.get_access_token().await?;
//! ```

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::GeminiError;

// ============================================================================
// Constants
// ============================================================================

/// Google OAuth token endpoint.
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Default client ID for gcloud.
#[allow(dead_code)]
const GCLOUD_CLIENT_ID: &str = "764086051850-6qr4p6gpi6hn506pt8ejuq83di341hur.apps.googleusercontent.com";

/// Default client secret for gcloud (not really secret).
#[allow(dead_code)]
const GCLOUD_CLIENT_SECRET: &str = "d-FL95Q19q7MQmFpd7hHD0Ty";

// ============================================================================
// Credential Types
// ============================================================================

/// Application Default Credentials file format.
#[derive(Debug, Deserialize)]
pub struct AdcCredentials {
    /// OAuth client ID.
    pub client_id: String,

    /// OAuth client secret.
    pub client_secret: String,

    /// Refresh token for obtaining new access tokens.
    pub refresh_token: String,

    /// Credential type (usually "authorized_user").
    #[serde(rename = "type")]
    pub cred_type: String,

    /// Quota project (optional).
    #[serde(default)]
    pub quota_project_id: Option<String>,
}

/// gcloud access token.
#[derive(Debug, Clone)]
pub struct GcloudToken {
    /// The OAuth access token.
    pub access_token: String,

    /// When the token expires (if known).
    pub expires_at: Option<DateTime<Utc>>,

    /// The account this token is for.
    pub account: Option<String>,

    /// The project (if known).
    pub project: Option<String>,
}

impl GcloudToken {
    /// Check if the token is expired.
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|exp| exp < Utc::now() + chrono::Duration::minutes(5))
    }
}

/// Token response from Google OAuth.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<u64>,
    #[allow(dead_code)]
    token_type: Option<String>,
}

// ============================================================================
// Credentials Database
// ============================================================================

/// Entry from credentials.db.
#[derive(Debug)]
struct CredentialsDbEntry {
    account: String,
    access_token: String,
    #[allow(dead_code)]
    token_expiry: Option<String>,
}

// ============================================================================
// gcloud Credentials
// ============================================================================

/// gcloud credential reader.
#[derive(Debug, Clone, Default)]
pub struct GcloudCredentials;

impl GcloudCredentials {
    /// Creates a new gcloud credentials reader.
    pub fn new() -> Self {
        Self
    }

    /// Load access token from any available source.
    ///
    /// Priority:
    /// 1. CLI (`gcloud auth print-access-token`)
    /// 2. Credentials database
    /// 3. ADC with refresh
    #[instrument(skip(self))]
    pub async fn load(&self) -> Result<GcloudToken, GeminiError> {
        // Try CLI first (most reliable)
        if let Ok(token) = self.get_from_cli().await {
            debug!(source = "cli", "Got token from gcloud CLI");
            return Ok(token);
        }

        // Try credentials database
        if let Ok(token) = self.load_from_db() {
            if !token.is_expired() {
                debug!(source = "db", "Got token from credentials.db");
                return Ok(token);
            }
        }

        // Try ADC with refresh
        if let Ok(token) = self.load_from_adc().await {
            debug!(source = "adc", "Got token from ADC refresh");
            return Ok(token);
        }

        Err(GeminiError::NoCredentials)
    }

    /// Check if gcloud CLI is available.
    pub fn is_cli_available() -> bool {
        which::which("gcloud").is_ok()
    }

    /// Get access token from gcloud CLI.
    #[instrument(skip(self))]
    pub async fn get_from_cli(&self) -> Result<GcloudToken, GeminiError> {
        debug!("Getting token from gcloud CLI");

        if !Self::is_cli_available() {
            return Err(GeminiError::GcloudNotFound);
        }

        // Get access token
        let output = tokio::process::Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .await
            .map_err(|e| GeminiError::GcloudError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GeminiError::GcloudError(stderr.to_string()));
        }

        let access_token = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if access_token.is_empty() {
            return Err(GeminiError::GcloudError("Empty token".to_string()));
        }

        // Get account info
        let account = self.get_account_from_cli().await.ok();
        let project = self.get_project_from_cli().await.ok();

        Ok(GcloudToken {
            access_token,
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)), // Assume 1 hour
            account,
            project,
        })
    }

    /// Get current account from gcloud CLI.
    async fn get_account_from_cli(&self) -> Result<String, GeminiError> {
        let output = tokio::process::Command::new("gcloud")
            .args(["config", "get-value", "account"])
            .output()
            .await
            .map_err(|e| GeminiError::GcloudError(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(GeminiError::GcloudError("Could not get account".to_string()))
        }
    }

    /// Get current project from gcloud CLI.
    async fn get_project_from_cli(&self) -> Result<String, GeminiError> {
        let output = tokio::process::Command::new("gcloud")
            .args(["config", "get-value", "project"])
            .output()
            .await
            .map_err(|e| GeminiError::GcloudError(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(GeminiError::GcloudError("Could not get project".to_string()))
        }
    }

    /// Load token from ADC file and refresh if needed.
    #[instrument(skip(self))]
    pub async fn load_from_adc(&self) -> Result<GcloudToken, GeminiError> {
        debug!("Loading from ADC");

        let adc_path = Self::adc_path().ok_or(GeminiError::AdcNotFound)?;

        if !adc_path.exists() {
            return Err(GeminiError::AdcNotFound);
        }

        let content = std::fs::read_to_string(&adc_path)
            .map_err(|e| GeminiError::CredentialsParseError(e.to_string()))?;

        let adc: AdcCredentials = serde_json::from_str(&content)
            .map_err(|e| GeminiError::CredentialsParseError(e.to_string()))?;

        // Refresh the token
        self.refresh_token(&adc).await
    }

    /// Refresh an access token using a refresh token.
    async fn refresh_token(&self, adc: &AdcCredentials) -> Result<GcloudToken, GeminiError> {
        debug!("Refreshing access token");

        let client = reqwest::Client::new();

        let params = [
            ("client_id", adc.client_id.as_str()),
            ("client_secret", adc.client_secret.as_str()),
            ("refresh_token", adc.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ];

        let response = client
            .post(GOOGLE_TOKEN_URL)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GeminiError::RefreshFailed(body));
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            GeminiError::CredentialsParseError(format!("Token response parse error: {}", e))
        })?;

        let expires_at = token_response
            .expires_in
            .map(|secs| Utc::now() + chrono::Duration::seconds(secs as i64));

        Ok(GcloudToken {
            access_token: token_response.access_token,
            expires_at,
            account: None,
            project: adc.quota_project_id.clone(),
        })
    }

    /// Load token from credentials.db SQLite database.
    #[instrument(skip(self))]
    pub fn load_from_db(&self) -> Result<GcloudToken, GeminiError> {
        debug!("Loading from credentials.db");

        let db_path = Self::credentials_db_path().ok_or(GeminiError::NoCredentials)?;

        if !db_path.exists() {
            return Err(GeminiError::NoCredentials);
        }

        // Copy to temp to avoid locking
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("gcloud_creds_{}.db", std::process::id()));

        std::fs::copy(&db_path, &temp_path)
            .map_err(|e| GeminiError::CredentialsParseError(e.to_string()))?;

        let conn = rusqlite::Connection::open(&temp_path)
            .map_err(|e| GeminiError::CredentialsParseError(e.to_string()))?;

        // Try to read credentials table
        let mut stmt = conn
            .prepare(
                "SELECT account_id, access_token, token_expiry FROM credentials ORDER BY rowid DESC LIMIT 1",
            )
            .map_err(|e| GeminiError::CredentialsParseError(e.to_string()))?;

        let entry = stmt
            .query_row([], |row| {
                Ok(CredentialsDbEntry {
                    account: row.get(0)?,
                    access_token: row.get(1)?,
                    token_expiry: row.get(2).ok(),
                })
            })
            .map_err(|e| GeminiError::CredentialsParseError(e.to_string()))?;

        // Clean up
        let _ = std::fs::remove_file(&temp_path);

        Ok(GcloudToken {
            access_token: entry.access_token,
            expires_at: None, // Could parse token_expiry if needed
            account: Some(entry.account),
            project: None,
        })
    }

    /// Get the path to ADC file.
    pub fn adc_path() -> Option<PathBuf> {
        // Try GOOGLE_APPLICATION_CREDENTIALS env var first
        if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            return Some(PathBuf::from(path));
        }

        // Default location
        let config_dir = dirs::config_dir()?;
        Some(config_dir.join("gcloud").join("application_default_credentials.json"))
    }

    /// Get the path to credentials.db.
    pub fn credentials_db_path() -> Option<PathBuf> {
        let config_dir = dirs::config_dir()?;
        Some(config_dir.join("gcloud").join("credentials.db"))
    }

    /// Get the gcloud config directory.
    pub fn config_dir() -> Option<PathBuf> {
        let config_dir = dirs::config_dir()?;
        Some(config_dir.join("gcloud"))
    }

    /// Check if ADC file exists.
    pub fn has_adc() -> bool {
        Self::adc_path().is_some_and(|p| p.exists())
    }

    /// Check if credentials database exists.
    pub fn has_credentials_db() -> bool {
        Self::credentials_db_path().is_some_and(|p| p.exists())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_creation() {
        let creds = GcloudCredentials::new();
        assert!(std::mem::size_of_val(&creds) == 0); // Zero-sized type
    }

    #[test]
    fn test_adc_path() {
        let path = GcloudCredentials::adc_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("application_default_credentials.json"));
    }

    #[test]
    fn test_credentials_db_path() {
        let path = GcloudCredentials::credentials_db_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("credentials.db"));
    }

    #[test]
    fn test_is_cli_available() {
        // Just test the function runs
        let _ = GcloudCredentials::is_cli_available();
    }

    #[test]
    fn test_has_adc() {
        // Just test the function runs
        let _ = GcloudCredentials::has_adc();
    }

    #[test]
    fn test_parse_adc() {
        let json = r#"{
            "client_id": "123.apps.googleusercontent.com",
            "client_secret": "secret",
            "refresh_token": "1//refresh",
            "type": "authorized_user",
            "quota_project_id": "my-project"
        }"#;

        let adc: AdcCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(adc.cred_type, "authorized_user");
        assert_eq!(adc.quota_project_id, Some("my-project".to_string()));
    }

    #[test]
    fn test_token_is_expired() {
        let expired = GcloudToken {
            access_token: "token".to_string(),
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)),
            account: None,
            project: None,
        };
        assert!(expired.is_expired());

        let valid = GcloudToken {
            access_token: "token".to_string(),
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
            account: None,
            project: None,
        };
        assert!(!valid.is_expired());
    }
}
