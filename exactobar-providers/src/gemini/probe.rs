//! Gemini local credentials and API probe.
//!
//! This module handles reading OAuth credentials from the Gemini CLI config
//! files (~/.gemini/), refreshing expired tokens, and fetching quota data
//! from the Cloud Code Private API.
//!
//! ## Config Files
//!
//! - `~/.gemini/oauth_creds.json` - OAuth credentials (access/refresh tokens)
//! - `~/.gemini/settings.json` - Auth type settings
//!
//! ## Auth Types
//!
//! The Gemini CLI supports multiple auth types:
//! - `oauth-personal` - Personal OAuth (supported)
//! - `api-key` - API key (not supported for quota fetch)
//! - `vertex-ai` - Vertex AI (not supported for quota fetch)

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::PathBuf;
use tracing::{debug, info, warn};

use super::error::GeminiError;
use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};

// ============================================================================
// Path Helpers
// ============================================================================

/// Get the user's home directory.
fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Get the path to the OAuth credentials file.
fn credentials_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".gemini").join("oauth_creds.json"))
}

/// Get the path to the settings file.
fn settings_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".gemini").join("settings.json"))
}

// ============================================================================
// Auth Types
// ============================================================================

/// Gemini authentication type (from settings.json).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiAuthType {
    /// Personal OAuth (supported).
    OAuthPersonal,
    /// API key (not supported for quota fetch).
    ApiKey,
    /// Vertex AI (not supported for quota fetch).
    VertexAI,
    /// Unknown or not set.
    Unknown,
}

impl GeminiAuthType {
    /// Read the auth type from settings.json.
    pub fn from_settings() -> Self {
        let Some(path) = settings_path() else {
            debug!("No settings path available");
            return Self::Unknown;
        };

        let Ok(content) = std::fs::read_to_string(&path) else {
            debug!(path = %path.display(), "Could not read settings file");
            return Self::Unknown;
        };

        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            warn!("Failed to parse settings.json");
            return Self::Unknown;
        };

        // Navigate: settings.security.auth.selectedType
        let selected = json
            .get("security")
            .and_then(|s| s.get("auth"))
            .and_then(|a| a.get("selectedType"))
            .and_then(|t| t.as_str());

        match selected {
            Some("oauth-personal") => Self::OAuthPersonal,
            Some("api-key") => Self::ApiKey,
            Some("vertex-ai") => Self::VertexAI,
            other => {
                debug!(auth_type = ?other, "Unknown auth type in settings");
                Self::Unknown
            }
        }
    }

    /// Check if this auth type is supported for quota fetching.
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::OAuthPersonal | Self::Unknown)
    }
}

// ============================================================================
// OAuth Credentials
// ============================================================================

/// OAuth credentials from ~/.gemini/oauth_creds.json.
#[derive(Debug, Deserialize)]
pub struct GeminiCredentials {
    /// The access token for API calls.
    pub access_token: Option<String>,
    /// The refresh token for getting new access tokens.
    pub refresh_token: Option<String>,
    /// Token expiry time as Unix timestamp in milliseconds.
    pub expiry_date: Option<i64>,
    /// OAuth client ID.
    pub client_id: Option<String>,
    /// OAuth client secret.
    pub client_secret: Option<String>,
}

impl GeminiCredentials {
    /// Load credentials from ~/.gemini/oauth_creds.json.
    pub fn load() -> Result<Self, GeminiError> {
        let path = credentials_path().ok_or(GeminiError::NotLoggedIn)?;

        debug!(path = %path.display(), "Loading Gemini credentials");

        let content =
            std::fs::read_to_string(&path).map_err(|_| GeminiError::NotLoggedIn)?;

        serde_json::from_str(&content)
            .map_err(|e| GeminiError::CredentialsParseError(e.to_string()))
    }

    /// Check if the credentials file exists.
    pub fn exists() -> bool {
        credentials_path()
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    /// Check if the access token is expired.
    pub fn is_expired(&self) -> bool {
        let Some(expiry_ms) = self.expiry_date else {
            // No expiry date = assume expired
            return true;
        };
        let now_ms = chrono::Utc::now().timestamp_millis();
        expiry_ms < now_ms
    }

    /// Check if we have a refresh token.
    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token
            .as_ref()
            .is_some_and(|t| !t.is_empty())
    }

    /// Check if we have a valid access token.
    pub fn has_access_token(&self) -> bool {
        self.access_token
            .as_ref()
            .is_some_and(|t| !t.is_empty())
    }
}

// ============================================================================
// API Endpoints
// ============================================================================

/// Cloud Code Private API endpoint for quota retrieval.
const QUOTA_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";

/// Google OAuth token refresh endpoint.
const TOKEN_REFRESH_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

// ============================================================================
// API Response Types
// ============================================================================

/// Response from the quota endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaResponse {
    /// List of model quota usages.
    model_quota_usages: Option<Vec<ModelQuotaUsage>>,
}

/// Individual model quota usage.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelQuotaUsage {
    /// Model identifier (e.g., "gemini-2.0-flash", "gemini-2.5-pro").
    model_id: Option<String>,
    /// Remaining requests in the current period.
    remaining_requests: Option<i64>,
    /// Total request limit for the period.
    request_limit: Option<i64>,
    /// When the quota resets (RFC3339 timestamp).
    requests_reset_time: Option<String>,
}

/// Token refresh response.
#[derive(Debug, Deserialize)]
struct TokenRefreshResponse {
    /// The new access token.
    access_token: String,
    /// Token expiry in seconds (optional).
    #[allow(dead_code)]
    expires_in: Option<u64>,
}

// ============================================================================
// Model Quota (Public)
// ============================================================================

/// Quota information for a single Gemini model.
#[derive(Debug, Clone)]
pub struct GeminiModelQuota {
    /// Model identifier (e.g., "gemini-2.0-flash").
    pub model_id: String,
    /// Percentage of quota remaining (0-100).
    pub percent_left: f64,
    /// When the quota resets.
    pub reset_time: Option<DateTime<Utc>>,
}

impl GeminiModelQuota {
    /// Get the percentage used (100 - percent_left).
    pub fn percent_used(&self) -> f64 {
        100.0 - self.percent_left
    }

    /// Check if this is a Pro model.
    pub fn is_pro(&self) -> bool {
        self.model_id.to_lowercase().contains("pro")
    }

    /// Check if this is a Flash model.
    pub fn is_flash(&self) -> bool {
        self.model_id.to_lowercase().contains("flash")
    }
}

// ============================================================================
// Snapshot (Public)
// ============================================================================

/// Snapshot of Gemini quota data.
#[derive(Debug)]
pub struct GeminiSnapshot {
    /// Per-model quota information.
    pub model_quotas: Vec<GeminiModelQuota>,
    /// Account email (if known).
    pub account_email: Option<String>,
    /// Account plan (if known).
    pub account_plan: Option<String>,
}

impl GeminiSnapshot {
    /// Check if we have any quota data.
    pub fn has_data(&self) -> bool {
        !self.model_quotas.is_empty()
    }

    /// Find the Pro model quota.
    pub fn pro_quota(&self) -> Option<&GeminiModelQuota> {
        self.model_quotas.iter().find(|q| q.is_pro())
    }

    /// Find the Flash model quota.
    pub fn flash_quota(&self) -> Option<&GeminiModelQuota> {
        self.model_quotas.iter().find(|q| q.is_flash())
    }

    /// Convert to a UsageSnapshot for the unified interface.
    pub fn to_usage_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::OAuth;

        // Primary = Pro model (the more expensive/limited one)
        if let Some(pro) = self.pro_quota() {
            snapshot.primary = Some(UsageWindow {
                used_percent: pro.percent_used(),
                window_minutes: Some(1440), // 24 hours
                resets_at: pro.reset_time,
                reset_description: Some(format!("Pro ({})", pro.model_id)),
            });
        }

        // Secondary = Flash model
        if let Some(flash) = self.flash_quota() {
            snapshot.secondary = Some(UsageWindow {
                used_percent: flash.percent_used(),
                window_minutes: Some(1440), // 24 hours
                resets_at: flash.reset_time,
                reset_description: Some(format!("Flash ({})", flash.model_id)),
            });
        }

        // Build identity
        let mut identity = ProviderIdentity::new(ProviderKind::Gemini);
        identity.account_email = self.account_email.clone();
        identity.plan_name = self.account_plan.clone();
        identity.login_method = Some(LoginMethod::OAuth);
        snapshot.identity = Some(identity);

        snapshot
    }
}

// ============================================================================
// Probe
// ============================================================================

/// Gemini quota probe using local OAuth credentials.
///
/// This probe reads credentials from ~/.gemini/oauth_creds.json,
/// refreshes expired tokens, and fetches quota data from the
/// Cloud Code Private API.
pub struct GeminiProbe {
    http: reqwest::Client,
}

impl GeminiProbe {
    /// Create a new Gemini probe.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { http }
    }

    /// Check if Gemini CLI credentials are available.
    pub fn is_available() -> bool {
        GeminiCredentials::exists()
    }

    /// Fetch quota data from the Gemini API.
    pub async fn fetch(&self) -> Result<GeminiSnapshot, GeminiError> {
        // Check auth type first
        let auth_type = GeminiAuthType::from_settings();
        debug!(auth_type = ?auth_type, "Detected Gemini auth type");

        match auth_type {
            GeminiAuthType::ApiKey => {
                return Err(GeminiError::UnsupportedAuthType(
                    "API key auth does not support quota queries".to_string(),
                ));
            }
            GeminiAuthType::VertexAI => {
                return Err(GeminiError::UnsupportedAuthType(
                    "Vertex AI auth requires different endpoint".to_string(),
                ));
            }
            _ => {}
        }

        // Load credentials
        let creds = GeminiCredentials::load()?;

        // Get a valid access token (refreshing if needed)
        let access_token = self.get_valid_token(&creds).await?;

        // Fetch quotas from the API
        self.fetch_quotas(&access_token).await
    }

    /// Get a valid access token, refreshing if expired.
    async fn get_valid_token(&self, creds: &GeminiCredentials) -> Result<String, GeminiError> {
        let token = creds
            .access_token
            .as_ref()
            .filter(|t| !t.is_empty())
            .ok_or(GeminiError::NotLoggedIn)?;

        // If token is expired and we have a refresh token, refresh it
        if creds.is_expired() {
            if creds.has_refresh_token() {
                info!("Gemini access token expired, refreshing...");
                return self.refresh_token(creds).await;
            } else {
                warn!("Gemini token expired but no refresh token available");
                return Err(GeminiError::TokenExpired(
                    "No refresh token available".to_string(),
                ));
            }
        }

        Ok(token.clone())
    }

    /// Refresh the access token using the refresh token.
    async fn refresh_token(&self, creds: &GeminiCredentials) -> Result<String, GeminiError> {
        let refresh_token = creds
            .refresh_token
            .as_ref()
            .ok_or(GeminiError::NotLoggedIn)?;

        let client_id = creds
            .client_id
            .as_ref()
            .ok_or_else(|| GeminiError::CredentialsParseError("Missing client_id".to_string()))?;

        let client_secret = creds.client_secret.as_ref().ok_or_else(|| {
            GeminiError::CredentialsParseError("Missing client_secret".to_string())
        })?;

        debug!("Refreshing Gemini OAuth token");

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ];

        let response = self
            .http
            .post(TOKEN_REFRESH_ENDPOINT)
            .form(&params)
            .send()
            .await
            .map_err(|e| GeminiError::HttpError(format!("Token refresh request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GeminiError::RefreshFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let refresh_response: TokenRefreshResponse = response
            .json()
            .await
            .map_err(|e| GeminiError::CredentialsParseError(format!("Invalid refresh response: {}", e)))?;

        info!("Successfully refreshed Gemini OAuth token");

        // Note: We could save the new token back to oauth_creds.json here,
        // but that might interfere with the Gemini CLI. Let's just use it.

        Ok(refresh_response.access_token)
    }

    /// Fetch quota information from the Cloud Code Private API.
    async fn fetch_quotas(&self, access_token: &str) -> Result<GeminiSnapshot, GeminiError> {
        debug!("Fetching Gemini quotas from Cloud Code API");

        let response = self
            .http
            .post(QUOTA_ENDPOINT)
            .bearer_auth(access_token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| GeminiError::HttpError(format!("Quota request failed: {}", e)))?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(GeminiError::NotLoggedIn);
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GeminiError::InvalidResponse(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let quota_response: QuotaResponse = response.json().await.map_err(|e| {
            GeminiError::InvalidResponse(format!("Failed to parse quota response: {}", e))
        })?;

        // Parse model quotas
        let model_quotas = quota_response
            .model_quota_usages
            .unwrap_or_default()
            .into_iter()
            .filter_map(|m| {
                let model_id = m.model_id?;
                let remaining = m.remaining_requests.unwrap_or(0) as f64;
                let limit = m.request_limit.unwrap_or(1) as f64;

                let percent_left = if limit > 0.0 {
                    (remaining / limit) * 100.0
                } else {
                    100.0
                };

                let reset_time = m
                    .requests_reset_time
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc));

                debug!(
                    model = %model_id,
                    remaining = remaining,
                    limit = limit,
                    percent_left = percent_left,
                    "Parsed model quota"
                );

                Some(GeminiModelQuota {
                    model_id,
                    percent_left,
                    reset_time,
                })
            })
            .collect();

        Ok(GeminiSnapshot {
            model_quotas,
            account_email: None, // Could extract from JWT token if needed
            account_plan: None,
        })
    }
}

impl Default for GeminiProbe {
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
    fn test_auth_type_supported() {
        assert!(GeminiAuthType::OAuthPersonal.is_supported());
        assert!(GeminiAuthType::Unknown.is_supported());
        assert!(!GeminiAuthType::ApiKey.is_supported());
        assert!(!GeminiAuthType::VertexAI.is_supported());
    }

    #[test]
    fn test_credentials_expired() {
        // Expired token (1 hour ago)
        let expired_ms = chrono::Utc::now().timestamp_millis() - 3600 * 1000;
        let creds = GeminiCredentials {
            access_token: Some("token".to_string()),
            refresh_token: Some("refresh".to_string()),
            expiry_date: Some(expired_ms),
            client_id: None,
            client_secret: None,
        };
        assert!(creds.is_expired());

        // Valid token (1 hour from now)
        let valid_ms = chrono::Utc::now().timestamp_millis() + 3600 * 1000;
        let creds = GeminiCredentials {
            access_token: Some("token".to_string()),
            refresh_token: Some("refresh".to_string()),
            expiry_date: Some(valid_ms),
            client_id: None,
            client_secret: None,
        };
        assert!(!creds.is_expired());

        // No expiry date = expired
        let creds = GeminiCredentials {
            access_token: Some("token".to_string()),
            refresh_token: None,
            expiry_date: None,
            client_id: None,
            client_secret: None,
        };
        assert!(creds.is_expired());
    }

    #[test]
    fn test_model_quota_type_detection() {
        let pro = GeminiModelQuota {
            model_id: "gemini-2.5-pro".to_string(),
            percent_left: 80.0,
            reset_time: None,
        };
        assert!(pro.is_pro());
        assert!(!pro.is_flash());

        let flash = GeminiModelQuota {
            model_id: "gemini-2.0-flash".to_string(),
            percent_left: 90.0,
            reset_time: None,
        };
        assert!(!flash.is_pro());
        assert!(flash.is_flash());
    }

    #[test]
    fn test_snapshot_to_usage_snapshot() {
        let snapshot = GeminiSnapshot {
            model_quotas: vec![
                GeminiModelQuota {
                    model_id: "gemini-2.5-pro".to_string(),
                    percent_left: 75.0, // 25% used
                    reset_time: None,
                },
                GeminiModelQuota {
                    model_id: "gemini-2.0-flash".to_string(),
                    percent_left: 90.0, // 10% used
                    reset_time: None,
                },
            ],
            account_email: Some("test@example.com".to_string()),
            account_plan: None,
        };

        let usage = snapshot.to_usage_snapshot();

        // Primary should be Pro (25% used)
        assert!(usage.primary.is_some());
        assert_eq!(usage.primary.as_ref().unwrap().used_percent, 25.0);

        // Secondary should be Flash (10% used)
        assert!(usage.secondary.is_some());
        assert_eq!(usage.secondary.as_ref().unwrap().used_percent, 10.0);

        // Identity should have email
        assert!(usage.identity.is_some());
        assert_eq!(
            usage.identity.as_ref().unwrap().account_email,
            Some("test@example.com".to_string())
        );
    }

    #[test]
    fn test_probe_creation() {
        let probe = GeminiProbe::new();
        // Just verify it doesn't panic
        assert!(std::mem::size_of_val(&probe) > 0);
    }
}
