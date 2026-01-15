//! Codex authentication utilities.
//!
//! This module handles reading authentication data from `~/.codex/auth.json`
//! and extracting account information from JWT tokens.
//!
//! # Auth.json Format
//!
//! ```json
//! {
//!   "tokens": {
//!     "idToken": "eyJ..."
//!   }
//! }
//! ```
//!
//! # JWT Payload
//!
//! The idToken JWT contains:
//! - `email` - Account email
//! - `https://api.openai.com/auth` - Object with `chatgpt_plan_type`

use base64::prelude::*;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, instrument, trace, warn};

use super::error::CodexError;

// ============================================================================
// Auth File Structures
// ============================================================================

/// Root structure of auth.json.
#[derive(Debug, Deserialize)]
pub struct AuthFile {
    /// Token container.
    pub tokens: Option<TokenContainer>,
}

/// Container for various tokens.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenContainer {
    /// The ID token (JWT).
    pub id_token: Option<String>,
    /// Access token.
    #[allow(dead_code)]
    pub access_token: Option<String>,
    /// Refresh token.
    #[allow(dead_code)]
    pub refresh_token: Option<String>,
}

// ============================================================================
// JWT Payload
// ============================================================================

/// JWT payload extracted from the ID token.
#[derive(Debug, Deserialize)]
pub struct JwtPayload {
    /// Subject (user ID).
    pub sub: Option<String>,
    /// Email address.
    pub email: Option<String>,
    /// Email verified flag.
    #[allow(dead_code)]
    pub email_verified: Option<bool>,
    /// Name.
    #[allow(dead_code)]
    pub name: Option<String>,
    /// Picture URL.
    #[allow(dead_code)]
    pub picture: Option<String>,
    /// Issued at timestamp.
    #[allow(dead_code)]
    pub iat: Option<i64>,
    /// Expiration timestamp.
    pub exp: Option<i64>,
    /// OpenAI-specific auth data.
    #[serde(rename = "https://api.openai.com/auth")]
    pub openai_auth: Option<OpenAiAuthData>,
}

/// OpenAI-specific authentication data embedded in the JWT.
#[derive(Debug, Deserialize)]
pub struct OpenAiAuthData {
    /// ChatGPT plan type (e.g., "free", "plus", "pro").
    pub chatgpt_plan_type: Option<String>,
    /// User ID.
    #[allow(dead_code)]
    pub user_id: Option<String>,
    /// Organizations.
    pub organizations: Option<Vec<OrgInfo>>,
}

/// Organization information.
#[derive(Debug, Deserialize)]
pub struct OrgInfo {
    /// Organization ID.
    #[allow(dead_code)]
    pub id: Option<String>,
    /// Organization name.
    pub name: Option<String>,
    /// Role in the organization.
    #[allow(dead_code)]
    pub role: Option<String>,
}

// ============================================================================
// Account Info (Simplified)
// ============================================================================

/// Simplified account information extracted from auth data.
#[derive(Debug, Clone, Default)]
pub struct AccountInfo {
    /// Account email.
    pub email: Option<String>,
    /// Plan type (e.g., "plus", "pro").
    pub plan: Option<String>,
    /// User ID.
    pub user_id: Option<String>,
    /// Primary organization name.
    pub organization: Option<String>,
    /// Whether the token is expired.
    pub is_expired: bool,
}

impl AccountInfo {
    /// Returns true if we have useful account info.
    pub fn has_data(&self) -> bool {
        self.email.is_some() || self.plan.is_some()
    }
}

// ============================================================================
// Auth Functions
// ============================================================================

/// Returns the path to the Codex auth file.
pub fn auth_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("auth.json"))
}

/// Check if the auth file exists.
#[allow(dead_code)]
pub fn auth_file_exists() -> bool {
    auth_file_path().is_some_and(|p| p.exists())
}

/// Read and parse the auth.json file.
#[instrument]
pub fn read_auth_file() -> Result<AuthFile, CodexError> {
    let path = auth_file_path()
        .ok_or_else(|| CodexError::AuthNotFound("Could not determine home directory".to_string()))?;

    if !path.exists() {
        return Err(CodexError::AuthNotFound(path.display().to_string()));
    }

    debug!(path = %path.display(), "Reading auth file");

    let content = fs::read_to_string(&path).map_err(|e| {
        warn!(error = %e, "Failed to read auth file");
        CodexError::IoError(e.to_string())
    })?;

    let auth: AuthFile = serde_json::from_str(&content).map_err(|e| {
        warn!(error = %e, "Failed to parse auth file");
        CodexError::InvalidAuth(e.to_string())
    })?;

    Ok(auth)
}

/// Extract the ID token from the auth file.
pub fn get_id_token() -> Result<String, CodexError> {
    let auth = read_auth_file()?;

    auth.tokens
        .and_then(|t| t.id_token)
        .ok_or(CodexError::InvalidAuth("No ID token found".to_string()))
}

/// Decode a JWT and extract the payload.
///
/// Note: This does NOT validate the JWT signature - we're just extracting
/// the payload for reading account info.
#[instrument(skip(token))]
pub fn decode_jwt_payload(token: &str) -> Result<JwtPayload, CodexError> {
    // JWT format: header.payload.signature
    let parts: Vec<&str> = token.split('.').collect();

    if parts.len() != 3 {
        return Err(CodexError::JwtError(format!(
            "Invalid JWT format: expected 3 parts, got {}",
            parts.len()
        )));
    }

    let payload_b64 = parts[1];

    // JWT uses base64url encoding (URL-safe, no padding)
    let decoded = BASE64_URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| {
            // Try with standard base64 as fallback
            BASE64_STANDARD.decode(payload_b64)
        })
        .map_err(|e| CodexError::JwtError(format!("Base64 decode error: {}", e)))?;

    let payload_str = String::from_utf8(decoded)
        .map_err(|e| CodexError::JwtError(format!("UTF-8 decode error: {}", e)))?;

    trace!(payload = %payload_str, "Decoded JWT payload");

    let payload: JwtPayload = serde_json::from_str(&payload_str)
        .map_err(|e| CodexError::JwtError(format!("JSON parse error: {}", e)))?;

    Ok(payload)
}

/// Read account information from the auth file.
#[instrument]
pub fn read_account_info() -> Result<AccountInfo, CodexError> {
    let token = get_id_token()?;
    let payload = decode_jwt_payload(&token)?;

    let mut info = AccountInfo::default();

    info.email = payload.email;
    info.user_id = payload.sub;

    // Check expiration
    if let Some(exp) = payload.exp {
        let now = chrono::Utc::now().timestamp();
        info.is_expired = now > exp;
        if info.is_expired {
            debug!(exp = exp, now = now, "Token is expired");
        }
    }

    // Extract OpenAI-specific data
    if let Some(openai) = payload.openai_auth {
        info.plan = openai.chatgpt_plan_type;

        // Get primary organization
        if let Some(orgs) = openai.organizations {
            if let Some(first_org) = orgs.into_iter().next() {
                info.organization = first_org.name;
            }
        }
    }

    debug!(
        email = ?info.email,
        plan = ?info.plan,
        expired = info.is_expired,
        "Account info extracted"
    );

    Ok(info)
}

/// Try to read account info, returning None on any error.
pub fn try_read_account_info() -> Option<AccountInfo> {
    match read_account_info() {
        Ok(info) if info.has_data() => Some(info),
        Ok(_) => {
            debug!("Auth file exists but contains no useful data");
            None
        }
        Err(e) => {
            debug!(error = %e, "Failed to read account info");
            None
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_auth_file() {
        let json = r#"{
            "tokens": {
                "idToken": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwiZW1haWwiOiJ0ZXN0QGV4YW1wbGUuY29tIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
                "accessToken": "access123",
                "refreshToken": "refresh456"
            }
        }"#;

        let auth: AuthFile = serde_json::from_str(json).unwrap();
        assert!(auth.tokens.is_some());

        let tokens = auth.tokens.unwrap();
        assert!(tokens.id_token.is_some());
        assert_eq!(tokens.access_token, Some("access123".to_string()));
    }

    #[test]
    fn test_decode_jwt_simple() {
        // A simple test JWT with email
        // Header: {"alg":"HS256","typ":"JWT"}
        // Payload: {"sub":"1234567890","email":"test@example.com"}
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwiZW1haWwiOiJ0ZXN0QGV4YW1wbGUuY29tIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

        let payload = decode_jwt_payload(token).unwrap();
        assert_eq!(payload.sub, Some("1234567890".to_string()));
        assert_eq!(payload.email, Some("test@example.com".to_string()));
    }

    #[test]
    fn test_decode_jwt_with_openai_auth() {
        // Create a JWT with OpenAI auth data
        // Payload: {"email":"user@test.com","https://api.openai.com/auth":{"chatgpt_plan_type":"plus"}}
        let payload_json = r#"{"email":"user@test.com","https://api.openai.com/auth":{"chatgpt_plan_type":"plus"}}"#;
        let payload_b64 = BASE64_URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let token = format!("eyJhbGciOiJIUzI1NiJ9.{}.signature", payload_b64);

        let payload = decode_jwt_payload(&token).unwrap();
        assert_eq!(payload.email, Some("user@test.com".to_string()));
        assert!(payload.openai_auth.is_some());
        assert_eq!(
            payload.openai_auth.unwrap().chatgpt_plan_type,
            Some("plus".to_string())
        );
    }

    #[test]
    fn test_decode_jwt_invalid() {
        // Invalid format
        assert!(decode_jwt_payload("not.a.valid.jwt").is_err());
        assert!(decode_jwt_payload("only_one_part").is_err());
        assert!(decode_jwt_payload("").is_err());
    }

    #[test]
    fn test_account_info_has_data() {
        let empty = AccountInfo::default();
        assert!(!empty.has_data());

        let with_email = AccountInfo {
            email: Some("test@example.com".to_string()),
            ..Default::default()
        };
        assert!(with_email.has_data());

        let with_plan = AccountInfo {
            plan: Some("plus".to_string()),
            ..Default::default()
        };
        assert!(with_plan.has_data());
    }

    #[test]
    fn test_auth_file_path() {
        // Should return some path on most systems
        let path = auth_file_path();
        assert!(path.is_some());

        let path = path.unwrap();
        assert!(path.ends_with("auth.json"));
        assert!(path.to_string_lossy().contains(".codex"));
    }
}
