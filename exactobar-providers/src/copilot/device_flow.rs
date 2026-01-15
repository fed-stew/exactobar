//! GitHub Device Flow OAuth implementation.
//!
//! This module implements the OAuth 2.0 Device Authorization Grant
//! (RFC 8628) for GitHub, which is used by Copilot for authentication.
//!
//! ## Flow
//!
//! 1. **Start**: POST to `/login/device/code` to get device code and user code
//! 2. **Display**: Show user the verification URL and user code
//! 3. **Poll**: POST to `/login/oauth/access_token` until user authorizes
//! 4. **Complete**: Store the access token
//!
//! ## Example
//!
//! ```ignore
//! let flow = CopilotDeviceFlow::new();
//! let start = flow.start().await?;
//! println!("Go to {} and enter code: {}", start.verification_uri, start.user_code);
//! 
//! loop {
//!     match flow.poll(&start.device_code).await? {
//!         DeviceFlowResult::Pending => tokio::time::sleep(Duration::from_secs(start.interval)).await,
//!         DeviceFlowResult::AccessToken(token) => break token,
//!         DeviceFlowResult::Expired => return Err("Expired".into()),
//!     }
//! }
//! ```

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use super::error::CopilotError;

// ============================================================================
// Constants
// ============================================================================

/// GitHub's OAuth device code endpoint.
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";

/// GitHub's OAuth access token endpoint.
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

/// GitHub Copilot's OAuth client ID.
/// This is the official client ID used by GitHub Copilot extensions.
const COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

/// Alternative client ID (VS Code Copilot Chat).
#[allow(dead_code)]
const COPILOT_CHAT_CLIENT_ID: &str = "Iv1.ae1de02f1d1c1a3a";

/// OAuth scopes required for Copilot.
const COPILOT_SCOPES: &str = "copilot";

// ============================================================================
// Types
// ============================================================================

/// Device flow start response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowStart {
    /// The device verification code.
    pub device_code: String,

    /// The user verification code to display.
    pub user_code: String,

    /// The verification URL.
    pub verification_uri: String,

    /// Seconds until the codes expire.
    pub expires_in: u64,

    /// Minimum polling interval in seconds.
    pub interval: u64,
}

/// Device flow poll result.
#[derive(Debug, Clone)]
pub enum DeviceFlowResult {
    /// User has not yet authorized - keep polling.
    Pending,

    /// User authorized - here's the access token.
    AccessToken(AccessTokenResponse),

    /// The device code expired.
    Expired,

    /// Access was denied by the user.
    AccessDenied,

    /// Polling too fast - slow down.
    SlowDown,
}

/// Access token response from GitHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenResponse {
    /// The OAuth access token.
    pub access_token: String,

    /// Token type (usually "bearer").
    pub token_type: String,

    /// Scopes granted.
    pub scope: String,
}

/// Error response from GitHub OAuth.
#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
    #[allow(dead_code)]
    error_description: Option<String>,
}

// ============================================================================
// Device Flow
// ============================================================================

/// GitHub Device Flow OAuth implementation for Copilot.
#[derive(Debug, Clone)]
pub struct CopilotDeviceFlow {
    http: reqwest::Client,
    client_id: String,
}

impl CopilotDeviceFlow {
    /// Creates a new device flow handler.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            client_id: COPILOT_CLIENT_ID.to_string(),
        }
    }

    /// Creates a device flow with a custom client ID.
    pub fn with_client_id(client_id: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            client_id: client_id.into(),
        }
    }

    /// Build headers for GitHub OAuth requests.
    fn build_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        headers
    }

    /// Start the device flow.
    ///
    /// Returns a device code and user code. The user must visit the
    /// verification URL and enter the user code to authorize.
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<DeviceFlowStart, CopilotError> {
        debug!("Starting GitHub device flow");

        let body = format!("client_id={}&scope={}", self.client_id, COPILOT_SCOPES);

        let response = self
            .http
            .post(DEVICE_CODE_URL)
            .headers(Self::build_headers())
            .body(body)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(CopilotError::DeviceFlowFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let body = response.text().await?;
        debug!(len = body.len(), "Got device code response");

        let start: DeviceFlowStart = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, body = %body, "Failed to parse device code response");
            CopilotError::InvalidResponse(format!("JSON parse error: {}", e))
        })?;

        debug!(
            user_code = %start.user_code,
            verification_uri = %start.verification_uri,
            expires_in = start.expires_in,
            "Device flow started"
        );

        Ok(start)
    }

    /// Poll for authorization completion.
    ///
    /// Call this repeatedly with the `interval` from `DeviceFlowStart`
    /// until you get `AccessToken`, `Expired`, or `AccessDenied`.
    #[instrument(skip(self, device_code))]
    pub async fn poll(&self, device_code: &str) -> Result<DeviceFlowResult, CopilotError> {
        debug!("Polling for device flow completion");

        let body = format!(
            "client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
            self.client_id, device_code
        );

        let response = self
            .http
            .post(ACCESS_TOKEN_URL)
            .headers(Self::build_headers())
            .body(body)
            .send()
            .await?;

        let body = response.text().await?;

        // Try to parse as success first
        if let Ok(token_response) = serde_json::from_str::<AccessTokenResponse>(&body) {
            debug!("Device flow completed - got access token");
            return Ok(DeviceFlowResult::AccessToken(token_response));
        }

        // Try to parse as error
        if let Ok(error_response) = serde_json::from_str::<OAuthError>(&body) {
            match error_response.error.as_str() {
                "authorization_pending" => {
                    debug!("Authorization pending");
                    return Ok(DeviceFlowResult::Pending);
                }
                "slow_down" => {
                    debug!("Polling too fast");
                    return Ok(DeviceFlowResult::SlowDown);
                }
                "expired_token" => {
                    debug!("Device code expired");
                    return Ok(DeviceFlowResult::Expired);
                }
                "access_denied" => {
                    debug!("Access denied by user");
                    return Ok(DeviceFlowResult::AccessDenied);
                }
                _ => {
                    warn!(error = %error_response.error, "Unknown OAuth error");
                    return Err(CopilotError::DeviceFlowFailed(error_response.error));
                }
            }
        }

        Err(CopilotError::InvalidResponse(format!(
            "Unexpected response: {}",
            body
        )))
    }

    /// Run the complete device flow with user interaction.
    ///
    /// This is a convenience method that handles the full flow:
    /// 1. Starts the device flow
    /// 2. Calls the provided callback with verification URL and user code
    /// 3. Polls until completion or timeout
    ///
    /// Returns the access token on success.
    #[instrument(skip(self, on_start))]
    pub async fn run_with_callback<F>(
        &self,
        on_start: F,
    ) -> Result<AccessTokenResponse, CopilotError>
    where
        F: FnOnce(&DeviceFlowStart),
    {
        let start = self.start().await?;
        on_start(&start);

        let interval = std::time::Duration::from_secs(start.interval.max(5));
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(start.expires_in);

        loop {
            if std::time::Instant::now() > deadline {
                return Err(CopilotError::DeviceFlowExpired);
            }

            tokio::time::sleep(interval).await;

            match self.poll(&start.device_code).await? {
                DeviceFlowResult::Pending => continue,
                DeviceFlowResult::SlowDown => {
                    // Wait extra time
                    tokio::time::sleep(interval).await;
                    continue;
                }
                DeviceFlowResult::AccessToken(token) => return Ok(token),
                DeviceFlowResult::Expired => return Err(CopilotError::DeviceFlowExpired),
                DeviceFlowResult::AccessDenied => {
                    return Err(CopilotError::AuthenticationFailed(
                        "User denied access".to_string(),
                    ))
                }
            }
        }
    }
}

impl Default for CopilotDeviceFlow {
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
    fn test_device_flow_creation() {
        let flow = CopilotDeviceFlow::new();
        assert_eq!(flow.client_id, COPILOT_CLIENT_ID);
    }

    #[test]
    fn test_device_flow_custom_client_id() {
        let flow = CopilotDeviceFlow::with_client_id("custom_id");
        assert_eq!(flow.client_id, "custom_id");
    }

    #[test]
    fn test_parse_device_code_response() {
        let json = r#"{
            "device_code": "3584d83530557fdd1f46af8289938c8ef79f9dc5",
            "user_code": "WDJB-MJHT",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 900,
            "interval": 5
        }"#;

        let start: DeviceFlowStart = serde_json::from_str(json).unwrap();
        assert_eq!(start.user_code, "WDJB-MJHT");
        assert_eq!(start.verification_uri, "https://github.com/login/device");
        assert_eq!(start.expires_in, 900);
        assert_eq!(start.interval, 5);
    }

    #[test]
    fn test_parse_access_token_response() {
        let json = r#"{
            "access_token": "gho_abc123",
            "token_type": "bearer",
            "scope": "copilot"
        }"#;

        let token: AccessTokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(token.access_token, "gho_abc123");
        assert_eq!(token.token_type, "bearer");
        assert_eq!(token.scope, "copilot");
    }

    #[test]
    fn test_parse_oauth_error() {
        let json = r#"{
            "error": "authorization_pending",
            "error_description": "The authorization request is still pending."
        }"#;

        let error: OAuthError = serde_json::from_str(json).unwrap();
        assert_eq!(error.error, "authorization_pending");
    }
}
