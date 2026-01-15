//! JSON-RPC client for Codex app-server.
//!
//! This module provides a client for communicating with the Codex CLI
//! in app-server mode via JSON-RPC over stdin/stdout.
//!
//! # Protocol
//!
//! The Codex app-server accepts JSON-RPC 2.0 messages:
//!
//! ```bash
//! codex -s read-only -a untrusted app-server
//! ```
//!
//! Messages are newline-delimited JSON.
//!
//! # Example
//!
//! ```ignore
//! let mut client = CodexRpcClient::spawn().await?;
//! client.initialize().await?;
//! let limits = client.fetch_rate_limits().await?;
//! client.shutdown();
//! ```

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, instrument, trace, warn};

use super::error::CodexError;

// ============================================================================
// Constants
// ============================================================================

/// Command to start Codex app-server.
const CODEX_BINARY: &str = "codex";

/// Arguments for app-server mode.
const APP_SERVER_ARGS: &[&str] = &["-s", "read-only", "-a", "untrusted", "app-server"];

/// Timeout for individual RPC requests.
const RPC_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for spawning the process.
#[allow(dead_code)]
const SPAWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Client name for initialization.
const CLIENT_NAME: &str = "exactobar";

/// Client version for initialization.
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// JSON-RPC Messages
// ============================================================================

/// JSON-RPC request message.
#[derive(Debug, Serialize)]
struct RpcRequest<'a, T: Serialize> {
    jsonrpc: &'static str,
    id: u32,
    method: &'a str,
    params: T,
}

impl<'a, T: Serialize> RpcRequest<'a, T> {
    fn new(id: u32, method: &'a str, params: T) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method,
            params,
        }
    }
}

/// JSON-RPC response message.
#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u32,
    result: Option<T>,
    error: Option<RpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
    #[allow(dead_code)]
    data: Option<serde_json::Value>,
}

// ============================================================================
// Initialize Request/Response
// ============================================================================

/// Parameters for the initialize request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    client_info: ClientInfo,
}

#[derive(Debug, Serialize)]
struct ClientInfo {
    name: &'static str,
    version: &'static str,
}

/// Result from initialize request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Server capabilities.
    #[allow(dead_code)]
    pub capabilities: Option<serde_json::Value>,
    /// Server info.
    pub server_info: Option<ServerInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ServerInfo {
    pub name: Option<String>,
    pub version: Option<String>,
}

// ============================================================================
// Rate Limits Request/Response
// ============================================================================

/// Empty params for rate limits request.
#[derive(Debug, Serialize)]
struct EmptyParams {}

/// Result from account/rateLimits/read request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitsResult {
    /// Rate limits data.
    pub rate_limits: RateLimits,
}

/// Rate limits container.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimits {
    /// Primary rate limit (5-hour window).
    pub primary: Option<RateLimitWindow>,
    /// Secondary rate limit (weekly window).
    pub secondary: Option<RateLimitWindow>,
    /// Credits information.
    pub credits: Option<CreditsInfo>,
}

/// Individual rate limit window.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitWindow {
    /// Percentage of limit used (0-100).
    pub used_percent: f64,
    /// Window duration in minutes.
    pub window_duration_mins: Option<u32>,
    /// Unix timestamp when the window resets.
    pub resets_at: Option<i64>,
}

/// Credits information.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditsInfo {
    /// Whether the account has credits.
    pub has_credits: Option<bool>,
    /// Whether credits are unlimited.
    pub unlimited: Option<bool>,
    /// Credit balance as string (to preserve precision).
    pub balance: Option<String>,
}

// ============================================================================
// Account Request/Response
// ============================================================================

/// Result from account/read request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResult {
    /// Account email.
    pub email: Option<String>,
    /// Organization name.
    pub organization: Option<String>,
    /// Plan type.
    pub plan: Option<String>,
}

// ============================================================================
// RPC Client
// ============================================================================

/// JSON-RPC client for Codex app-server.
pub struct CodexRpcClient {
    /// Child process running app-server.
    child: Child,
    /// Stdin writer.
    writer: ChildStdin,
    /// Stdout reader.
    reader: BufReader<ChildStdout>,
    /// Next request ID.
    next_id: AtomicU32,
    /// Whether initialized.
    initialized: bool,
    /// Server version from initialization.
    server_version: Option<String>,
}

impl CodexRpcClient {
    /// Spawn a new Codex app-server process and create a client.
    #[instrument]
    pub fn spawn() -> Result<Self, CodexError> {
        debug!("Spawning Codex app-server");

        // Check if codex exists
        if which::which(CODEX_BINARY).is_err() {
            return Err(CodexError::BinaryNotFound(CODEX_BINARY.to_string()));
        }

        // Spawn the process
        let start = Instant::now();
        let mut child = Command::new(CODEX_BINARY)
            .args(APP_SERVER_ARGS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CodexError::SpawnFailed(e.to_string()))?;

        let writer = child
            .stdin
            .take()
            .ok_or_else(|| CodexError::SpawnFailed("Failed to get stdin".to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CodexError::SpawnFailed("Failed to get stdout".to_string()))?;

        let reader = BufReader::new(stdout);

        debug!(elapsed = ?start.elapsed(), "App-server spawned");

        Ok(Self {
            child,
            writer,
            reader,
            next_id: AtomicU32::new(1),
            initialized: false,
            server_version: None,
        })
    }

    /// Initialize the RPC connection.
    #[instrument(skip(self))]
    pub fn initialize(&mut self) -> Result<InitializeResult, CodexError> {
        debug!("Initializing RPC connection");

        let params = InitializeParams {
            client_info: ClientInfo {
                name: CLIENT_NAME,
                version: CLIENT_VERSION,
            },
        };

        let result: InitializeResult = self.call("initialize", params)?;

        // Store server version
        if let Some(ref info) = result.server_info {
            self.server_version = info.version.clone();
            debug!(version = ?info.version, "Server initialized");
        }

        self.initialized = true;
        Ok(result)
    }

    /// Fetch rate limits from the server.
    #[instrument(skip(self))]
    pub fn fetch_rate_limits(&mut self) -> Result<RateLimitsResult, CodexError> {
        if !self.initialized {
            return Err(CodexError::NotInitialized);
        }

        debug!("Fetching rate limits");
        self.call("account/rateLimits/read", EmptyParams {})
    }

    /// Fetch account information from the server.
    #[instrument(skip(self))]
    pub fn fetch_account(&mut self) -> Result<AccountResult, CodexError> {
        if !self.initialized {
            return Err(CodexError::NotInitialized);
        }

        debug!("Fetching account info");
        self.call("account/read", EmptyParams {})
    }

    /// Returns the server version, if known.
    pub fn server_version(&self) -> Option<&str> {
        self.server_version.as_deref()
    }

    /// Shutdown the RPC client and terminate the process.
    pub fn shutdown(&mut self) {
        debug!("Shutting down RPC client");

        // Try to send a shutdown notification (best effort)
        let _ = self.send_notification("shutdown", EmptyParams {});

        // Kill the child process
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Make an RPC call and wait for the response.
    fn call<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R, CodexError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = RpcRequest::new(id, method, params);

        // Send request
        self.send_request(&request)?;

        // Read response with timeout
        let start = Instant::now();
        loop {
            if start.elapsed() > RPC_TIMEOUT {
                return Err(CodexError::Timeout(RPC_TIMEOUT));
            }

            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => {
                    return Err(CodexError::ConnectionClosed);
                }
                Ok(_) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    trace!(line = %line, "Received line");

                    // Try to parse as our response
                    match serde_json::from_str::<RpcResponse<R>>(line) {
                        Ok(response) => {
                            if let Some(error) = response.error {
                                return Err(CodexError::RpcError {
                                    code: error.code,
                                    message: error.message,
                                });
                            }
                            return response.result.ok_or(CodexError::EmptyResponse);
                        }
                        Err(e) => {
                            // Could be a notification or other message, continue reading
                            trace!(error = %e, "Failed to parse response, continuing");
                        }
                    }
                }
                Err(e) => {
                    return Err(CodexError::IoError(e.to_string()));
                }
            }
        }
    }

    /// Send an RPC request.
    fn send_request<T: Serialize>(&mut self, request: &RpcRequest<T>) -> Result<(), CodexError> {
        let json = serde_json::to_string(request)
            .map_err(|e| CodexError::SerializationError(e.to_string()))?;

        trace!(json = %json, "Sending request");

        writeln!(self.writer, "{}", json).map_err(|e| CodexError::IoError(e.to_string()))?;

        self.writer
            .flush()
            .map_err(|e| CodexError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Send a notification (no response expected).
    fn send_notification<T: Serialize>(
        &mut self,
        method: &str,
        params: T,
    ) -> Result<(), CodexError> {
        #[derive(Serialize)]
        struct Notification<'a, T> {
            jsonrpc: &'static str,
            method: &'a str,
            params: T,
        }

        let notification = Notification {
            jsonrpc: "2.0",
            method,
            params,
        };

        let json = serde_json::to_string(&notification)
            .map_err(|e| CodexError::SerializationError(e.to_string()))?;

        writeln!(self.writer, "{}", json).map_err(|e| CodexError::IoError(e.to_string()))?;

        self.writer
            .flush()
            .map_err(|e| CodexError::IoError(e.to_string()))?;

        Ok(())
    }
}

impl Drop for CodexRpcClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limits_deserialize() {
        let json = r#"{
            "rateLimits": {
                "primary": {
                    "usedPercent": 28.5,
                    "windowDurationMins": 300,
                    "resetsAt": 1735000000
                },
                "secondary": {
                    "usedPercent": 59.2,
                    "windowDurationMins": 10080,
                    "resetsAt": 1735100000
                },
                "credits": {
                    "hasCredits": true,
                    "unlimited": false,
                    "balance": "112.45"
                }
            }
        }"#;

        let result: RateLimitsResult = serde_json::from_str(json).unwrap();

        let primary = result.rate_limits.primary.unwrap();
        assert!((primary.used_percent - 28.5).abs() < 0.01);
        assert_eq!(primary.window_duration_mins, Some(300));

        let secondary = result.rate_limits.secondary.unwrap();
        assert!((secondary.used_percent - 59.2).abs() < 0.01);

        let credits = result.rate_limits.credits.unwrap();
        assert_eq!(credits.has_credits, Some(true));
        assert_eq!(credits.balance, Some("112.45".to_string()));
    }

    #[test]
    fn test_account_deserialize() {
        let json = r#"{
            "email": "user@example.com",
            "organization": "Acme Inc",
            "plan": "pro"
        }"#;

        let result: AccountResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.email, Some("user@example.com".to_string()));
        assert_eq!(result.organization, Some("Acme Inc".to_string()));
        assert_eq!(result.plan, Some("pro".to_string()));
    }

    #[test]
    fn test_initialize_deserialize() {
        let json = r#"{
            "capabilities": {},
            "serverInfo": {
                "name": "codex",
                "version": "1.2.3"
            }
        }"#;

        let result: InitializeResult = serde_json::from_str(json).unwrap();
        assert!(result.server_info.is_some());
        assert_eq!(
            result.server_info.unwrap().version,
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn test_rpc_error_deserialize() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32600,
                "message": "Invalid request"
            }
        }"#;

        let response: RpcResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32600);
    }
}
