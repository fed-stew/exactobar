//! MiniMax web API client.
//!
//! MiniMax (also known as Hailuoai) stores auth tokens in:
//! - Browser localStorage (hailuoai.com domain)
//! - Web cookies
//!
//! This module supports both authentication methods.

use std::path::PathBuf;

use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, COOKIE, USER_AGENT};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::MiniMaxError;

// ============================================================================
// Constants
// ============================================================================

/// MiniMax API base URL.
const MINIMAX_API_BASE: &str = "https://api.minimax.chat";

/// Hailuoai API base URL (alternate domain).
pub const HAILUOAI_API_BASE: &str = "https://hailuoai.com/api";

/// Usage endpoint.
const USAGE_ENDPOINT: &str = "/v1/usage";

/// Hailuoai usage endpoint.
pub const HAILUOAI_USAGE_ENDPOINT: &str = "/user/usage";

/// MiniMax cookie domain.
pub const MINIMAX_DOMAIN: &str = "minimax.chat";

/// Hailuoai cookie domain (MiniMax's web interface).
pub const HAILUOAI_DOMAIN: &str = "hailuoai.com";

/// Session cookie names for MiniMax.
const SESSION_COOKIE_NAMES: &[&str] = &[
    "__session",
    "minimax_session",
    "session",
];

/// Session cookie names specific to Hailuoai.
const HAILUOAI_COOKIE_NAMES: &[&str] = &[
    "_token",
    "user_token",
    "token",
    "access_token",
    "auth_token",
    "session",
    "__Secure-next-auth.session-token",
];

// ============================================================================
// API Response Types
// ============================================================================

/// Response from MiniMax usage API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniMaxUsageResponse {
    /// Tokens used.
    #[serde(default, alias = "tokens_used")]
    pub tokens_used: Option<u64>,

    /// Token limit.
    #[serde(default, alias = "token_limit")]
    pub token_limit: Option<u64>,

    /// Credits used.
    #[serde(default, alias = "credits_used")]
    pub credits_used: Option<f64>,

    /// Credit limit.
    #[serde(default, alias = "credit_limit")]
    pub credit_limit: Option<f64>,

    /// Balance.
    #[serde(default)]
    pub balance: Option<f64>,

    /// Reset time.
    #[serde(default, alias = "reset_at")]
    pub reset_at: Option<String>,

    /// Plan name.
    #[serde(default)]
    pub plan: Option<String>,

    /// User email.
    #[serde(default)]
    pub email: Option<String>,
}

impl MiniMaxUsageResponse {
    /// Get usage percentage.
    pub fn get_percent(&self) -> Option<f64> {
        // Try credits first
        if let (Some(used), Some(limit)) = (self.credits_used, self.credit_limit) {
            if limit > 0.0 {
                return Some((used / limit) * 100.0);
            }
        }

        // Try tokens
        if let (Some(used), Some(limit)) = (self.tokens_used, self.token_limit) {
            if limit > 0 {
                return Some((used as f64 / limit as f64) * 100.0);
            }
        }

        None
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self, source: FetchSource) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = source;

        if let Some(percent) = self.get_percent() {
            snapshot.primary = Some(UsageWindow::new(percent));
        }

        let mut identity = ProviderIdentity::new(ProviderKind::MiniMax);
        identity.account_email = self.email.clone();
        identity.plan_name = self.plan.clone();
        identity.login_method = match source {
            FetchSource::Web => Some(LoginMethod::BrowserCookies),
            _ => Some(LoginMethod::ApiKey),
        };
        snapshot.identity = Some(identity);

        snapshot
    }
}

/// Local storage token.
#[derive(Debug, Deserialize)]
pub struct LocalToken {
    /// API token.
    #[serde(default, alias = "api_token")]
    pub token: Option<String>,

    /// Expiry timestamp.
    #[serde(default, alias = "expires_at")]
    pub expires: Option<i64>,
}

// ============================================================================
// Browser LocalStorage Support
// ============================================================================

/// MiniMax localStorage reader for browser data.
///
/// MiniMax stores auth tokens in browser localStorage under the hailuoai.com domain.
/// This struct provides methods to locate and extract those tokens.
///
/// Note: Full localStorage parsing requires LevelDB support which is complex.
/// This is a best-effort implementation that may not work for all browsers.
#[derive(Debug, Clone, Default)]
pub struct MiniMaxLocalStorage;

impl MiniMaxLocalStorage {
    /// Paths to browser localStorage databases.
    ///
    /// Returns paths for Chrome, Arc, and Edge on macOS.
    /// Each browser stores localStorage in a LevelDB database.
    /// Returns candidate browser localStorage paths for MiniMax tokens.
    #[cfg(target_os = "macos")]
    pub fn local_storage_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(home) = dirs::home_dir() {
            let app_support = home.join("Library/Application Support");

            // Chrome
            paths.push(app_support.join("Google/Chrome/Default/Local Storage/leveldb"));

            // Chrome profiles
            for i in 1..=5 {
                paths.push(
                    app_support.join(format!("Google/Chrome/Profile {}/Local Storage/leveldb", i)),
                );
            }

            // Arc Browser
            paths.push(app_support.join("Arc/User Data/Default/Local Storage/leveldb"));

            // Edge
            paths.push(
                app_support.join("Microsoft Edge/Default/Local Storage/leveldb"),
            );

            // Brave
            paths.push(
                app_support.join("BraveSoftware/Brave-Browser/Default/Local Storage/leveldb"),
            );

            // Firefox uses a different storage mechanism (SQLite)
            // Not supported in this implementation
        }

        paths
    }

    /// Returns candidate browser localStorage paths for MiniMax tokens.
    #[cfg(target_os = "linux")]
    pub fn local_storage_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(config) = dirs::config_dir() {
            // Chrome
            paths.push(config.join("google-chrome/Default/Local Storage/leveldb"));

            // Chromium
            paths.push(config.join("chromium/Default/Local Storage/leveldb"));

            // Edge
            paths.push(config.join("microsoft-edge/Default/Local Storage/leveldb"));

            // Brave
            paths.push(config.join("BraveSoftware/Brave-Browser/Default/Local Storage/leveldb"));
        }

        paths
    }

    /// Returns candidate browser localStorage paths for MiniMax tokens.
    #[cfg(target_os = "windows")]
    pub fn local_storage_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(local_app_data) = dirs::data_local_dir() {
            // Chrome
            paths.push(
                local_app_data.join("Google/Chrome/User Data/Default/Local Storage/leveldb"),
            );

            // Edge
            paths.push(
                local_app_data.join("Microsoft/Edge/User Data/Default/Local Storage/leveldb"),
            );

            // Brave
            paths.push(
                local_app_data
                    .join("BraveSoftware/Brave-Browser/User Data/Default/Local Storage/leveldb"),
            );
        }

        paths
    }

    /// Check if any localStorage database exists.
    pub fn has_storage() -> bool {
        Self::local_storage_paths().iter().any(|p| p.exists())
    }

    /// Try to extract auth token from localStorage.
    ///
    /// This is a simplified implementation. Full LevelDB parsing is complex
    /// and would require the `leveldb` crate. For now, we attempt a basic
    /// string search in the LevelDB files.
    ///
    /// The primary authentication strategy should remain browser cookies.
    pub fn find_token() -> Option<String> {
        for path in Self::local_storage_paths() {
            if !path.exists() {
                continue;
            }

            debug!(path = %path.display(), "Searching localStorage for MiniMax token");

            // Try to find token in LevelDB log files
            if let Some(token) = Self::search_leveldb_logs(&path) {
                return Some(token);
            }
        }

        None
    }

    /// Search LevelDB log files for token patterns.
    ///
    /// LevelDB stores data in .log files that can sometimes be read as text.
    /// This is a best-effort approach.
    fn search_leveldb_logs(leveldb_path: &PathBuf) -> Option<String> {
        let log_path = leveldb_path.join("LOG");

        // Also check for .ldb files which contain the actual data
        if let Ok(entries) = std::fs::read_dir(leveldb_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "log" || e == "ldb") {
                    if let Some(token) = Self::extract_token_from_file(&path) {
                        return Some(token);
                    }
                }
            }
        }

        // Try the LOG file directly
        if log_path.exists() {
            if let Some(token) = Self::extract_token_from_file(&log_path) {
                return Some(token);
            }
        }

        None
    }

    /// Try to extract a token from a file by looking for hailuoai patterns.
    fn extract_token_from_file(path: &PathBuf) -> Option<String> {
        let content = std::fs::read(path).ok()?;

        // Convert to string, ignoring invalid UTF-8
        let text = String::from_utf8_lossy(&content);

        // Look for hailuoai.com localStorage entries
        // Format is typically: _https://hailuoai.com\x00\x01<key>\x00<value>
        if !text.contains("hailuoai") {
            return None;
        }

        // Try to find token patterns
        // Common patterns: "token":"...", "access_token":"...", etc.
        for pattern in ["\"token\":\"", "\"access_token\":\"", "\"auth_token\":\""] {
            if let Some(start) = text.find(pattern) {
                let value_start = start + pattern.len();
                if let Some(end) = text[value_start..].find('"') {
                    let token = &text[value_start..value_start + end];
                    // Basic validation: token should be reasonable length and alphanumeric-ish
                    if token.len() > 20 && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
                        debug!("Found potential MiniMax token in localStorage");
                        return Some(token.to_string());
                    }
                }
            }
        }

        None
    }

    /// Known localStorage keys that might contain MiniMax auth.
    pub fn known_token_keys() -> &'static [&'static str] {
        &[
            "token",
            "auth",
            "access_token",
            "user_token",
            "hailuoai_token",
            "minimax_token",
        ]
    }
}

// ============================================================================
// Token Store
// ============================================================================

/// MiniMax token store.
#[derive(Debug, Clone, Default)]
pub struct MiniMaxTokenStore;

impl MiniMaxTokenStore {
    /// Get local storage path.
    pub fn storage_path() -> Option<PathBuf> {
        let config_dir = dirs::config_dir()?;
        Some(config_dir.join("minimax").join("token.json"))
    }

    /// Load token from local storage.
    pub fn load() -> Option<String> {
        let path = Self::storage_path()?;
        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        let stored: LocalToken = serde_json::from_str(&content).ok()?;

        // Check expiry
        if let Some(expires) = stored.expires {
            let now = chrono::Utc::now().timestamp();
            if now > expires {
                return None; // Expired
            }
        }

        stored.token
    }

    /// Check if token is available.
    pub fn is_available() -> bool {
        Self::load().is_some()
    }
}

// ============================================================================
// Web Client
// ============================================================================

/// MiniMax web API client.
#[derive(Debug)]
pub struct MiniMaxWebClient {
    http: reqwest::Client,
}

impl MiniMaxWebClient {
    /// Creates a new client.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self { http }
    }

    /// Check for session cookie (MiniMax domain).
    pub fn has_session_cookie(cookie_header: &str) -> bool {
        SESSION_COOKIE_NAMES
            .iter()
            .any(|name| cookie_header.contains(name))
    }

    /// Check for session cookie (Hailuoai domain).
    pub fn has_hailuoai_session_cookie(cookie_header: &str) -> bool {
        HAILUOAI_COOKIE_NAMES
            .iter()
            .any(|name| cookie_header.contains(name))
    }

    /// Check for any valid session cookie (either domain).
    pub fn has_any_session_cookie(cookie_header: &str) -> bool {
        Self::has_session_cookie(cookie_header) || Self::has_hailuoai_session_cookie(cookie_header)
    }

    /// Build headers with cookie.
    fn build_cookie_headers(&self, cookie_header: &str) -> Result<HeaderMap, MiniMaxError> {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, HeaderValue::from_static("ExactoBar/1.0"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            COOKIE,
            HeaderValue::from_str(cookie_header)
                .map_err(|e| MiniMaxError::HttpError(format!("Invalid cookie: {}", e)))?,
        );

        Ok(headers)
    }

    /// Build headers with bearer token.
    fn build_token_headers(&self, token: &str) -> Result<HeaderMap, MiniMaxError> {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, HeaderValue::from_static("ExactoBar/1.0"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let auth_value = format!("Bearer {}", token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value)
                .map_err(|e| MiniMaxError::HttpError(format!("Invalid token: {}", e)))?,
        );

        Ok(headers)
    }

    /// Fetch usage with cookies.
    #[instrument(skip(self, cookie_header))]
    pub async fn fetch_usage_with_cookies(
        &self,
        cookie_header: &str,
    ) -> Result<MiniMaxUsageResponse, MiniMaxError> {
        debug!("Fetching MiniMax usage with cookies");

        let url = format!("{}{}", MINIMAX_API_BASE, USAGE_ENDPOINT);
        let headers = self.build_cookie_headers(cookie_header)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(MiniMaxError::AuthenticationFailed(
                "Session expired".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(MiniMaxError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let usage: MiniMaxUsageResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, "Failed to parse usage response");
            MiniMaxError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(usage)
    }

    /// Fetch usage with token.
    #[instrument(skip(self, token))]
    pub async fn fetch_usage_with_token(
        &self,
        token: &str,
    ) -> Result<MiniMaxUsageResponse, MiniMaxError> {
        debug!("Fetching MiniMax usage with token");

        let url = format!("{}{}", MINIMAX_API_BASE, USAGE_ENDPOINT);
        let headers = self.build_token_headers(token)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(MiniMaxError::AuthenticationFailed(
                "Token rejected".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(MiniMaxError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let usage: MiniMaxUsageResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, "Failed to parse usage response");
            MiniMaxError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(usage)
    }

    /// Fetch usage from Hailuoai domain with cookies.
    ///
    /// Hailuoai.com is MiniMax's web interface and may have different
    /// API endpoints than api.minimax.chat.
    #[instrument(skip(self, cookie_header))]
    pub async fn fetch_hailuoai_usage(
        &self,
        cookie_header: &str,
    ) -> Result<MiniMaxUsageResponse, MiniMaxError> {
        debug!("Fetching MiniMax usage from hailuoai.com");

        let url = format!("{}{}", HAILUOAI_API_BASE, HAILUOAI_USAGE_ENDPOINT);
        let headers = self.build_cookie_headers(cookie_header)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(MiniMaxError::AuthenticationFailed(
                "Hailuoai session expired".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(MiniMaxError::InvalidResponse(format!(
                "Hailuoai API returned HTTP {}",
                status
            )));
        }

        let body = response.text().await?;
        let usage: MiniMaxUsageResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, "Failed to parse hailuoai usage response");
            MiniMaxError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(usage)
    }

    /// Try localStorage token for authentication.
    ///
    /// Attempts to find and use a token from browser localStorage.
    #[instrument(skip(self))]
    pub async fn fetch_usage_from_local_storage(
        &self,
    ) -> Result<MiniMaxUsageResponse, MiniMaxError> {
        debug!("Attempting to fetch MiniMax usage from localStorage token");

        let token = MiniMaxLocalStorage::find_token()
            .ok_or_else(|| MiniMaxError::NoToken)?;

        self.fetch_usage_with_token(&token).await
    }
}

impl Default for MiniMaxWebClient {
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
    fn test_client_creation() {
        let client = MiniMaxWebClient::new();
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn test_has_session_cookie() {
        assert!(MiniMaxWebClient::has_session_cookie("__session=abc"));
        assert!(MiniMaxWebClient::has_session_cookie("minimax_session=xyz"));
        assert!(!MiniMaxWebClient::has_session_cookie("random=value"));
    }

    #[test]
    fn test_has_hailuoai_session_cookie() {
        assert!(MiniMaxWebClient::has_hailuoai_session_cookie("_token=abc123"));
        assert!(MiniMaxWebClient::has_hailuoai_session_cookie("user_token=xyz"));
        assert!(MiniMaxWebClient::has_hailuoai_session_cookie("access_token=foo"));
        assert!(!MiniMaxWebClient::has_hailuoai_session_cookie("random=value"));
    }

    #[test]
    fn test_has_any_session_cookie() {
        // MiniMax cookies
        assert!(MiniMaxWebClient::has_any_session_cookie("__session=abc"));
        // Hailuoai cookies
        assert!(MiniMaxWebClient::has_any_session_cookie("_token=abc123"));
        // Neither
        assert!(!MiniMaxWebClient::has_any_session_cookie("random=value"));
    }

    #[test]
    fn test_is_token_available() {
        let _ = MiniMaxTokenStore::is_available();
    }

    #[test]
    fn test_local_storage_paths() {
        let paths = MiniMaxLocalStorage::local_storage_paths();
        // Should return at least Chrome path
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_local_storage_has_storage() {
        // Just test it runs without error
        let _ = MiniMaxLocalStorage::has_storage();
    }

    #[test]
    fn test_local_storage_find_token() {
        // Just test it runs without error (likely returns None)
        let _ = MiniMaxLocalStorage::find_token();
    }

    #[test]
    fn test_known_token_keys() {
        let keys = MiniMaxLocalStorage::known_token_keys();
        assert!(keys.contains(&"token"));
        assert!(keys.contains(&"access_token"));
    }

    #[test]
    fn test_parse_usage_response() {
        let json = r#"{
            "creditsUsed": 50.0,
            "creditLimit": 100.0,
            "plan": "pro"
        }"#;

        let response: MiniMaxUsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.get_percent(), Some(50.0));
    }

    #[test]
    fn test_to_snapshot() {
        let response = MiniMaxUsageResponse {
            tokens_used: Some(500),
            token_limit: Some(1000),
            credits_used: None,
            credit_limit: None,
            balance: None,
            reset_at: None,
            plan: Some("pro".to_string()),
            email: Some("user@example.com".to_string()),
        };

        let snapshot = response.to_snapshot(FetchSource::Web);
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
    }

    #[test]
    fn test_domains_defined() {
        assert_eq!(MINIMAX_DOMAIN, "minimax.chat");
        assert_eq!(HAILUOAI_DOMAIN, "hailuoai.com");
    }
}
