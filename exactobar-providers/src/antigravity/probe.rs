//! Antigravity local language server probe.
//!
//! Detects running Antigravity process, extracts CSRF token,
//! and queries the gRPC-style API for usage quotas.

use chrono::{DateTime, Utc};
use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use std::process::Command;
use tracing::{debug, instrument};

use super::error::AntigravityError;

// ============================================================================
// Constants
// ============================================================================

const PROCESS_NAME: &str = "language_server_macos";
const GET_USER_STATUS_PATH: &str = "/exa.language_server_pb.LanguageServerService/GetUserStatus";
const GET_COMMAND_MODEL_PATH: &str = "/exa.language_server_pb.LanguageServerService/GetCommandModelConfigs";

// ============================================================================
// Process Detection
// ============================================================================

#[derive(Debug)]
struct ProcessInfo {
    pid: u32,
    csrf_token: String,
    extension_port: Option<u16>,
}

/// Detect running Antigravity process and extract CSRF token
fn detect_process() -> Result<ProcessInfo, AntigravityError> {
    let output = Command::new("/bin/ps")
        .args(["-ax", "-o", "pid=,command="])
        .output()
        .map_err(|_e| AntigravityError::NotRunning)?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse "PID command..."
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        if parts.len() != 2 {
            continue;
        }

        let pid: u32 = match parts[0].trim().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let command = parts[1];
        let lower = command.to_lowercase();

        // Check if this is Antigravity
        if !lower.contains(PROCESS_NAME) {
            continue;
        }
        if !is_antigravity_command(&lower) {
            continue;
        }

        // Extract CSRF token
        if let Some(token) = extract_flag("--csrf_token", command) {
            let port = extract_flag("--extension_server_port", command).and_then(|s| s.parse().ok());

            return Ok(ProcessInfo {
                pid,
                csrf_token: token,
                extension_port: port,
            });
        }
    }

    Err(AntigravityError::NotRunning)
}

fn is_antigravity_command(command: &str) -> bool {
    (command.contains("--app_data_dir") && command.contains("antigravity"))
        || command.contains("/antigravity/")
}

fn extract_flag(flag: &str, command: &str) -> Option<String> {
    // Match --flag=value or --flag value
    let patterns = [format!("{}=", flag), format!("{} ", flag)];

    for pattern in &patterns {
        if let Some(start) = command.find(pattern) {
            let value_start = start + pattern.len();
            let rest = &command[value_start..];
            let value_end = rest.find(' ').unwrap_or(rest.len());
            return Some(rest[..value_end].to_string());
        }
    }
    None
}

// ============================================================================
// Port Detection
// ============================================================================

fn detect_listening_ports(pid: u32) -> Result<Vec<u16>, AntigravityError> {
    let lsof_paths = ["/usr/sbin/lsof", "/usr/bin/lsof"];
    let lsof = lsof_paths
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .ok_or_else(|| AntigravityError::PortDetectionFailed("lsof not available".into()))?;

    let output = Command::new(lsof)
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-a", "-p", &pid.to_string()])
        .output()
        .map_err(|e| AntigravityError::PortDetectionFailed(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ports = Vec::new();

    // Parse lines like: "... :PORT (LISTEN)"
    for line in stdout.lines() {
        if let Some(port) = parse_port_from_lsof_line(line) {
            if !ports.contains(&port) {
                ports.push(port);
            }
        }
    }

    ports.sort();

    if ports.is_empty() {
        return Err(AntigravityError::PortDetectionFailed(
            "no listening ports found".into(),
        ));
    }

    Ok(ports)
}

fn parse_port_from_lsof_line(line: &str) -> Option<u16> {
    // Look for pattern like ":12345 (LISTEN)"
    let listen_idx = line.find("(LISTEN)")?;
    let before = &line[..listen_idx];
    let colon_idx = before.rfind(':')?;
    let port_str = before[colon_idx + 1..].trim();
    port_str.parse().ok()
}

// ============================================================================
// API Response Types (matching POC exactly)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserStatusResponse {
    code: Option<CodeValue>,
    #[allow(dead_code)]
    message: Option<String>,
    user_status: Option<UserStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandModelConfigResponse {
    code: Option<CodeValue>,
    #[allow(dead_code)]
    message: Option<String>,
    client_model_configs: Option<Vec<ModelConfig>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserStatus {
    email: Option<String>,
    plan_status: Option<PlanStatus>,
    cascade_model_config_data: Option<ModelConfigData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanStatus {
    plan_info: Option<PlanInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanInfo {
    plan_name: Option<String>,
    plan_display_name: Option<String>,
    display_name: Option<String>,
    product_name: Option<String>,
    #[allow(dead_code)]
    plan_short_name: Option<String>,
}

impl PlanInfo {
    fn preferred_name(&self) -> Option<&str> {
        self.plan_display_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self.display_name.as_deref().filter(|s| !s.is_empty()))
            .or(self.product_name.as_deref().filter(|s| !s.is_empty()))
            .or(self.plan_name.as_deref().filter(|s| !s.is_empty()))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelConfigData {
    client_model_configs: Option<Vec<ModelConfig>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelConfig {
    label: String,
    model_or_alias: ModelAlias,
    quota_info: Option<QuotaInfoResponse>,
}

#[derive(Debug, Deserialize)]
struct ModelAlias {
    model: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaInfoResponse {
    remaining_fraction: Option<f64>,
    reset_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CodeValue {
    Int(i32),
    String(String),
}

impl CodeValue {
    fn is_ok(&self) -> bool {
        match self {
            CodeValue::Int(v) => *v == 0,
            CodeValue::String(s) => {
                let lower = s.to_lowercase();
                lower == "ok" || lower == "success" || s == "0"
            }
        }
    }
}

// ============================================================================
// Model Quota Selection (matching POC logic)
// ============================================================================

/// Quota information for a specific model.
#[derive(Debug, Clone)]
pub struct ModelQuota {
    /// Human-readable label (e.g., "Claude without Thinking").
    pub label: String,
    /// Model identifier.
    pub model_id: String,
    /// Remaining fraction (0.0 to 1.0).
    pub remaining_fraction: Option<f64>,
    /// When this quota resets.
    pub reset_time: Option<DateTime<Utc>>,
}

impl ModelQuota {
    /// Get remaining percentage (0-100).
    pub fn remaining_percent(&self) -> f64 {
        self.remaining_fraction
            .map(|f| (f * 100.0).clamp(0.0, 100.0))
            .unwrap_or(0.0)
    }

    /// Get used percentage (0-100).
    pub fn used_percent(&self) -> f64 {
        100.0 - self.remaining_percent()
    }
}

/// Select and order models by priority (matching POC logic).
fn select_models(models: &[ModelQuota]) -> Vec<&ModelQuota> {
    let mut ordered = Vec::new();

    // Priority 1: Claude (without thinking)
    if let Some(claude) = models.iter().find(|m| is_claude_without_thinking(&m.label)) {
        ordered.push(claude);
    }

    // Priority 2: Gemini Pro Low
    if let Some(pro) = models.iter().find(|m| is_gemini_pro_low(&m.label)) {
        if !ordered.iter().any(|o| o.label == pro.label) {
            ordered.push(pro);
        }
    }

    // Priority 3: Gemini Flash
    if let Some(flash) = models.iter().find(|m| is_gemini_flash(&m.label)) {
        if !ordered.iter().any(|o| o.label == flash.label) {
            ordered.push(flash);
        }
    }

    // Fallback: sort by usage (most used first)
    if ordered.is_empty() {
        let mut sorted: Vec<_> = models.iter().collect();
        sorted.sort_by(|a, b| {
            b.used_percent()
                .partial_cmp(&a.used_percent())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        return sorted;
    }

    ordered
}

fn is_claude_without_thinking(label: &str) -> bool {
    let lower = label.to_lowercase();
    // Match Claude models that DON'T have "with thinking" in the name
    // "Claude without Thinking" is fine, "Claude with Thinking" is not
    lower.contains("claude") && !lower.contains("with thinking")
}

fn is_gemini_pro_low(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower.contains("pro") && lower.contains("low")
}

fn is_gemini_flash(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower.contains("gemini") && lower.contains("flash")
}

// ============================================================================
// Snapshot
// ============================================================================

/// Snapshot of Antigravity usage data.
#[derive(Debug)]
pub struct AntigravitySnapshot {
    /// Quotas for each model.
    pub model_quotas: Vec<ModelQuota>,
    /// Account email.
    pub account_email: Option<String>,
    /// Account plan name.
    pub account_plan: Option<String>,
}

impl AntigravitySnapshot {
    /// Convert to a UsageSnapshot for display.
    pub fn to_usage_snapshot(&self) -> Result<UsageSnapshot, AntigravityError> {
        let ordered = select_models(&self.model_quotas);
        let primary = ordered
            .first()
            .ok_or_else(|| AntigravityError::InvalidResponse("No quota models available".into()))?;

        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::LocalProbe;

        // Primary window (most important model - usually Claude)
        snapshot.primary = Some(UsageWindow {
            used_percent: primary.used_percent(),
            window_minutes: None,
            resets_at: primary.reset_time,
            reset_description: Some(primary.label.clone()),
        });

        // Secondary window
        if let Some(secondary) = ordered.get(1) {
            snapshot.secondary = Some(UsageWindow {
                used_percent: secondary.used_percent(),
                window_minutes: None,
                resets_at: secondary.reset_time,
                reset_description: Some(secondary.label.clone()),
            });
        }

        // Tertiary window
        if let Some(tertiary) = ordered.get(2) {
            snapshot.tertiary = Some(UsageWindow {
                used_percent: tertiary.used_percent(),
                window_minutes: None,
                resets_at: tertiary.reset_time,
                reset_description: Some(tertiary.label.clone()),
            });
        }

        // Identity
        let mut identity = ProviderIdentity::new(ProviderKind::Antigravity);
        identity.account_email = self.account_email.clone();
        identity.plan_name = self.account_plan.clone();
        identity.login_method = Some(LoginMethod::CLI);
        snapshot.identity = Some(identity);

        Ok(snapshot)
    }
}

// ============================================================================
// Probe Implementation
// ============================================================================

/// Antigravity local probe.
#[derive(Debug)]
pub struct AntigravityProbe {
    http: reqwest::Client,
}

impl AntigravityProbe {
    /// Create a new probe.
    pub fn new() -> Self {
        // Accept self-signed certs for localhost HTTPS
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .expect("Failed to build HTTP client");

        Self { http }
    }

    /// Check if Antigravity is running.
    #[instrument(skip(self))]
    pub async fn is_running(&self) -> bool {
        detect_process().is_ok()
    }

    /// Fetch usage data from Antigravity.
    #[instrument(skip(self))]
    pub async fn fetch(&self) -> Result<AntigravitySnapshot, AntigravityError> {
        let process = detect_process()?;
        debug!(pid = process.pid, "Found Antigravity process");

        let ports = if let Some(port) = process.extension_port {
            vec![port]
        } else {
            detect_listening_ports(process.pid)?
        };
        debug!(?ports, "Detected listening ports");

        let port = self.find_working_port(&ports, &process.csrf_token).await?;
        debug!(port, "Found working API port");

        // Try GetUserStatus first, fallback to GetCommandModelConfigs
        match self.fetch_user_status(port, &process.csrf_token).await {
            Ok(snapshot) => Ok(snapshot),
            Err(e) => {
                debug!(error = %e, "GetUserStatus failed, trying GetCommandModelConfigs");
                self.fetch_command_model_configs(port, &process.csrf_token)
                    .await
            }
        }
    }

    /// Fetch and convert to UsageSnapshot (convenience method).
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, AntigravityError> {
        let snapshot = self.fetch().await?;
        snapshot.to_usage_snapshot()
    }

    /// Find a working API port by testing connectivity.
    async fn find_working_port(
        &self,
        ports: &[u16],
        csrf_token: &str,
    ) -> Result<u16, AntigravityError> {
        for &port in ports {
            if self.test_port(port, csrf_token).await {
                return Ok(port);
            }
        }
        Err(AntigravityError::PortDetectionFailed(
            "no working API port".into(),
        ))
    }

    /// Quick connectivity test using GetUnleashData endpoint.
    async fn test_port(&self, port: u16, csrf_token: &str) -> bool {
        let url = format!(
            "https://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetUnleashData",
            port
        );
        let body = serde_json::json!({
            "context": {
                "properties": {
                    "ide": "antigravity",
                    "installationId": "exactobar"
                }
            }
        });

        self.post_request(&url, csrf_token, &body).await.is_ok()
    }

    /// Fetch user status from GetUserStatus endpoint.
    async fn fetch_user_status(
        &self,
        port: u16,
        csrf_token: &str,
    ) -> Result<AntigravitySnapshot, AntigravityError> {
        let url = format!("https://127.0.0.1:{}{}", port, GET_USER_STATUS_PATH);
        let body = default_request_body();

        let data = self.post_request(&url, csrf_token, &body).await?;
        let response: UserStatusResponse = serde_json::from_slice(&data)
            .map_err(|e| AntigravityError::InvalidResponse(e.to_string()))?;

        parse_user_status_response(response)
    }

    /// Fetch model configs from GetCommandModelConfigs endpoint.
    async fn fetch_command_model_configs(
        &self,
        port: u16,
        csrf_token: &str,
    ) -> Result<AntigravitySnapshot, AntigravityError> {
        let url = format!("https://127.0.0.1:{}{}", port, GET_COMMAND_MODEL_PATH);
        let body = default_request_body();

        let data = self.post_request(&url, csrf_token, &body).await?;
        let response: CommandModelConfigResponse = serde_json::from_slice(&data)
            .map_err(|e| AntigravityError::InvalidResponse(e.to_string()))?;

        parse_command_model_response(response)
    }

    /// Send a POST request to the API.
    async fn post_request(
        &self,
        url: &str,
        csrf_token: &str,
        body: &serde_json::Value,
    ) -> Result<Vec<u8>, AntigravityError> {
        let response = self
            .http
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .header("Connect-Protocol-Version", "1")
            .header("X-Codeium-Csrf-Token", csrf_token)
            .json(body)
            .send()
            .await
            .map_err(|e| AntigravityError::ConnectionFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AntigravityError::ApiError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| AntigravityError::InvalidResponse(e.to_string()))
    }
}

impl Default for AntigravityProbe {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Request/Response Helpers
// ============================================================================

fn default_request_body() -> serde_json::Value {
    serde_json::json!({
        "metadata": {
            "ideName": "antigravity",
            "extensionName": "antigravity",
            "ideVersion": "unknown",
            "locale": "en"
        }
    })
}

fn parse_user_status_response(
    response: UserStatusResponse,
) -> Result<AntigravitySnapshot, AntigravityError> {
    if let Some(code) = &response.code {
        if !code.is_ok() {
            return Err(AntigravityError::ApiError(format!("code: {:?}", code)));
        }
    }

    let user_status = response
        .user_status
        .ok_or_else(|| AntigravityError::InvalidResponse("Missing userStatus".into()))?;

    let model_configs = user_status
        .cascade_model_config_data
        .and_then(|d| d.client_model_configs)
        .unwrap_or_default();

    let models = model_configs
        .iter()
        .filter_map(quota_from_config)
        .collect();

    let plan_name = user_status
        .plan_status
        .and_then(|ps| ps.plan_info)
        .and_then(|pi| pi.preferred_name().map(String::from));

    Ok(AntigravitySnapshot {
        model_quotas: models,
        account_email: user_status.email,
        account_plan: plan_name,
    })
}

fn parse_command_model_response(
    response: CommandModelConfigResponse,
) -> Result<AntigravitySnapshot, AntigravityError> {
    if let Some(code) = &response.code {
        if !code.is_ok() {
            return Err(AntigravityError::ApiError(format!("code: {:?}", code)));
        }
    }

    let model_configs = response.client_model_configs.unwrap_or_default();
    let models = model_configs
        .iter()
        .filter_map(quota_from_config)
        .collect();

    Ok(AntigravitySnapshot {
        model_quotas: models,
        account_email: None,
        account_plan: None,
    })
}

fn quota_from_config(config: &ModelConfig) -> Option<ModelQuota> {
    let quota = config.quota_info.as_ref()?;

    let reset_time = quota.reset_time.as_ref().and_then(|s| parse_reset_time(s));

    Some(ModelQuota {
        label: config.label.clone(),
        model_id: config.model_or_alias.model.clone(),
        remaining_fraction: quota.remaining_fraction,
        reset_time,
    })
}

fn parse_reset_time(s: &str) -> Option<DateTime<Utc>> {
    // Try ISO8601/RFC3339 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // Try unix timestamp (seconds)
    if let Ok(secs) = s.parse::<i64>() {
        return DateTime::from_timestamp(secs, 0);
    }
    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_creation() {
        let _ = AntigravityProbe::new();
    }

    #[test]
    fn test_extract_flag_equals() {
        let cmd = "./server --csrf_token=abc123 --port=8080";
        assert_eq!(
            extract_flag("--csrf_token", cmd),
            Some("abc123".to_string())
        );
        assert_eq!(extract_flag("--port", cmd), Some("8080".to_string()));
    }

    #[test]
    fn test_extract_flag_space() {
        let cmd = "./server --csrf_token abc123 --port 8080";
        assert_eq!(
            extract_flag("--csrf_token", cmd),
            Some("abc123".to_string())
        );
        assert_eq!(extract_flag("--port", cmd), Some("8080".to_string()));
    }

    #[test]
    fn test_extract_flag_missing() {
        let cmd = "./server --other_flag=value";
        assert_eq!(extract_flag("--csrf_token", cmd), None);
    }

    #[test]
    fn test_is_antigravity_command() {
        assert!(is_antigravity_command(
            "--app_data_dir /path/antigravity/data"
        ));
        assert!(is_antigravity_command(
            "/applications/antigravity/bin/server"
        ));
        assert!(!is_antigravity_command("--app_data_dir /path/other/data"));
    }

    #[test]
    fn test_parse_port_from_lsof() {
        let line = "node    12345 user   23u  IPv4 0x123  0t0  TCP 127.0.0.1:42069 (LISTEN)";
        assert_eq!(parse_port_from_lsof_line(line), Some(42069));
    }

    #[test]
    fn test_parse_port_no_listen() {
        let line = "node    12345 user   23u  IPv4 0x123  0t0  TCP 127.0.0.1:42069";
        assert_eq!(parse_port_from_lsof_line(line), None);
    }

    #[test]
    fn test_model_quota_percent() {
        let quota = ModelQuota {
            label: "Claude".to_string(),
            model_id: "claude-3".to_string(),
            remaining_fraction: Some(0.75),
            reset_time: None,
        };
        assert_eq!(quota.remaining_percent(), 75.0);
        assert_eq!(quota.used_percent(), 25.0);
    }

    #[test]
    fn test_model_quota_no_fraction() {
        let quota = ModelQuota {
            label: "Claude".to_string(),
            model_id: "claude-3".to_string(),
            remaining_fraction: None,
            reset_time: None,
        };
        assert_eq!(quota.remaining_percent(), 0.0);
        assert_eq!(quota.used_percent(), 100.0);
    }

    #[test]
    fn test_select_models_claude_priority() {
        let models = vec![
            ModelQuota {
                label: "Gemini Flash".to_string(),
                model_id: "flash".to_string(),
                remaining_fraction: Some(0.5),
                reset_time: None,
            },
            ModelQuota {
                label: "Claude without Thinking".to_string(),
                model_id: "claude".to_string(),
                remaining_fraction: Some(0.3),
                reset_time: None,
            },
        ];
        let selected = select_models(&models);
        assert_eq!(selected[0].label, "Claude without Thinking");
    }

    #[test]
    fn test_is_claude_without_thinking() {
        assert!(is_claude_without_thinking("Claude without Thinking"));
        assert!(is_claude_without_thinking("claude"));
        assert!(!is_claude_without_thinking("Claude with Thinking"));
    }

    #[test]
    fn test_is_gemini_pro_low() {
        assert!(is_gemini_pro_low("Gemini Pro Low"));
        assert!(is_gemini_pro_low("Pro (Low Latency)"));
        assert!(!is_gemini_pro_low("Gemini Pro"));
    }

    #[test]
    fn test_is_gemini_flash() {
        assert!(is_gemini_flash("Gemini Flash"));
        assert!(is_gemini_flash("gemini 2.0 flash"));
        assert!(!is_gemini_flash("Gemini Pro"));
    }

    #[test]
    fn test_code_value_is_ok() {
        assert!(CodeValue::Int(0).is_ok());
        assert!(!CodeValue::Int(1).is_ok());
        assert!(CodeValue::String("ok".to_string()).is_ok());
        assert!(CodeValue::String("OK".to_string()).is_ok());
        assert!(CodeValue::String("success".to_string()).is_ok());
        assert!(!CodeValue::String("error".to_string()).is_ok());
    }

    #[test]
    fn test_parse_reset_time_rfc3339() {
        let result = parse_reset_time("2024-01-15T12:00:00Z");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_reset_time_unix() {
        let result = parse_reset_time("1705320000");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_reset_time_invalid() {
        let result = parse_reset_time("not-a-date");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_user_status_response() {
        let json = r#"{
            "code": 0,
            "userStatus": {
                "email": "test@example.com",
                "planStatus": {
                    "planInfo": {
                        "planDisplayName": "Pro Plan"
                    }
                },
                "cascadeModelConfigData": {
                    "clientModelConfigs": [
                        {
                            "label": "Claude without Thinking",
                            "modelOrAlias": { "model": "claude-3" },
                            "quotaInfo": { "remainingFraction": 0.75 }
                        }
                    ]
                }
            }
        }"#;

        let response: UserStatusResponse = serde_json::from_str(json).unwrap();
        let snapshot = parse_user_status_response(response).unwrap();

        assert_eq!(snapshot.account_email, Some("test@example.com".to_string()));
        assert_eq!(snapshot.account_plan, Some("Pro Plan".to_string()));
        assert_eq!(snapshot.model_quotas.len(), 1);
        assert_eq!(snapshot.model_quotas[0].remaining_percent(), 75.0);
    }

    #[test]
    fn test_snapshot_to_usage_snapshot() {
        let snapshot = AntigravitySnapshot {
            model_quotas: vec![ModelQuota {
                label: "Claude".to_string(),
                model_id: "claude-3".to_string(),
                remaining_fraction: Some(0.6),
                reset_time: None,
            }],
            account_email: Some("test@example.com".to_string()),
            account_plan: Some("Pro".to_string()),
        };

        let usage = snapshot.to_usage_snapshot().unwrap();
        assert!(usage.primary.is_some());
        assert_eq!(usage.primary.as_ref().unwrap().used_percent, 40.0);
        assert!(usage.identity.is_some());
    }
}
