//! Provider-related types.
//!
//! This module contains types related to LLM providers:
//! - [`ProviderKind`] - Enum of supported providers
//! - [`Provider`] - Provider configuration
//! - [`ProviderIdentity`] - Account identity (siloed per provider)
//! - [`ProviderMetadata`] - Provider capabilities and display info
//! - [`ProviderBranding`] - Visual styling

use serde::{Deserialize, Serialize};

// ============================================================================
// Provider Kind
// ============================================================================

/// Supported LLM provider kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    /// OpenAI Codex
    Codex,
    /// Anthropic Claude
    Claude,
    /// Cursor IDE
    Cursor,
    /// Google Gemini
    Gemini,
    /// GitHub Copilot
    Copilot,
    /// Factory AI
    Factory,
    /// Google Cloud Vertex AI
    VertexAI,
    /// z.ai
    Zai,
    /// Augment Code
    Augment,
    /// Kiro AI
    Kiro,
    /// Antigravity AI
    Antigravity,
    /// MiniMax
    MiniMax,
}

impl ProviderKind {
    /// Returns the display name for this provider.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Cursor => "Cursor",
            Self::Gemini => "Gemini",
            Self::Copilot => "Copilot",
            Self::Factory => "Factory",
            Self::VertexAI => "Vertex AI",
            Self::Zai => "z.ai",
            Self::Augment => "Augment",
            Self::Kiro => "Kiro",
            Self::Antigravity => "Antigravity",
            Self::MiniMax => "MiniMax",
        }
    }

    /// Returns all available provider kinds.
    pub fn all() -> &'static [ProviderKind] {
        &[
            Self::Codex,
            Self::Claude,
            Self::Cursor,
            Self::Gemini,
            Self::Copilot,
            Self::Factory,
            Self::VertexAI,
            Self::Zai,
            Self::Augment,
            Self::Kiro,
            Self::Antigravity,
            Self::MiniMax,
        ]
    }

    /// Returns the CLI name for this provider (lowercase, no spaces).
    pub fn cli_name(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
            Self::Gemini => "gemini",
            Self::Copilot => "copilot",
            Self::Factory => "factory",
            Self::VertexAI => "vertexai",
            Self::Zai => "zai",
            Self::Augment => "augment",
            Self::Kiro => "kiro",
            Self::Antigravity => "antigravity",
            Self::MiniMax => "minimax",
        }
    }

    /// Converts this provider to an index (position in the `all()` array).
    ///
    /// Useful for compact serialization, e.g., storing in Objective-C ivars.
    pub fn to_index(self) -> usize {
        Self::all()
            .iter()
            .position(|&p| p == self)
            .unwrap_or(0)
    }

    /// Creates a provider from an index (position in the `all()` array).
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn from_index(index: usize) -> Option<Self> {
        Self::all().get(index).copied()
    }
}

// ============================================================================
// Provider Configuration
// ============================================================================

/// Configuration for a specific provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// The kind of provider.
    pub kind: ProviderKind,
    /// Whether this provider is enabled.
    pub enabled: bool,
    /// Optional display name override.
    pub display_name: Option<String>,
    /// Environment variable name for the API key.
    pub api_key_env: Option<String>,
    /// Direct API key (not recommended, prefer api_key_env).
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
}

impl Provider {
    /// Creates a new provider with the given kind.
    pub fn new(kind: ProviderKind) -> Self {
        Self {
            kind,
            enabled: true,
            display_name: None,
            api_key_env: None,
            api_key: None,
        }
    }

    /// Returns the effective display name.
    pub fn effective_display_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(self.kind.display_name())
    }
}

// ============================================================================
// Provider Identity
// ============================================================================

/// Account identity information for a provider.
///
/// **Important**: This is siloed per provider - never mix identity from
/// different providers. Each provider has its own authentication context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderIdentity {
    /// The provider this identity belongs to.
    pub provider_id: ProviderKind,
    /// Account email address.
    pub account_email: Option<String>,
    /// Organization name (if applicable).
    pub account_organization: Option<String>,
    /// Plan/subscription name.
    pub plan_name: Option<String>,
    /// How the user authenticated.
    pub login_method: Option<LoginMethod>,
}

impl ProviderIdentity {
    /// Creates a new identity for the given provider.
    pub fn new(provider_id: ProviderKind) -> Self {
        Self {
            provider_id,
            account_email: None,
            account_organization: None,
            plan_name: None,
            login_method: None,
        }
    }

    /// Returns a display string for this identity.
    pub fn display_string(&self) -> String {
        match (&self.account_email, &self.account_organization) {
            (Some(email), Some(org)) => format!("{} ({})", email, org),
            (Some(email), None) => email.clone(),
            (None, Some(org)) => org.clone(),
            (None, None) => self.provider_id.display_name().to_string(),
        }
    }
}

/// How the user authenticated with a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoginMethod {
    /// OAuth 2.0 flow.
    OAuth,
    /// API key authentication.
    #[default]
    ApiKey,
    /// Browser cookies (scraped from browser).
    BrowserCookies,
    /// CLI tool authentication.
    CLI,
    /// Device flow (OAuth device authorization).
    DeviceFlow,
}

// ============================================================================
// Provider Metadata
// ============================================================================

/// Metadata describing a provider's capabilities and display info.
///
/// This is static configuration that describes what a provider supports
/// and how it should be displayed in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    /// The provider this metadata describes.
    pub id: ProviderKind,
    /// Display name in UI.
    pub display_name: String,
    /// Label for session/primary window (e.g., "Session", "5-hour").
    pub session_label: String,
    /// Label for weekly/secondary window (e.g., "Weekly", "Monthly").
    pub weekly_label: String,
    /// Label for opus/tertiary window (e.g., "Opus" for Claude).
    pub opus_label: Option<String>,
    /// Whether this provider supports opus/premium tier.
    pub supports_opus: bool,
    /// Whether this provider uses a credit system.
    pub supports_credits: bool,
    /// Hint text for credits display.
    pub credits_hint: String,
    /// Title for the toggle in settings (e.g., "Show Claude usage").
    pub toggle_title: String,
    /// CLI command name.
    pub cli_name: String,
    /// Whether enabled by default.
    pub default_enabled: bool,
    /// Whether this is considered a primary provider.
    pub is_primary_provider: bool,
    /// Whether to use account fallback for display.
    pub uses_account_fallback: bool,
    /// URL to the provider's dashboard.
    pub dashboard_url: Option<String>,
    /// URL to subscription/billing page.
    pub subscription_dashboard_url: Option<String>,
    /// URL to status page API.
    pub status_page_url: Option<String>,
    /// URL to status page for users.
    pub status_link_url: Option<String>,
}

impl ProviderMetadata {
    /// Creates default metadata for a provider kind.
    pub fn for_provider(kind: ProviderKind) -> Self {
        let name = kind.display_name();
        Self {
            id: kind,
            display_name: name.to_string(),
            session_label: "Session".to_string(),
            weekly_label: "Weekly".to_string(),
            opus_label: None,
            supports_opus: false,
            supports_credits: false,
            credits_hint: String::new(),
            toggle_title: format!("Show {} usage", name),
            cli_name: kind.cli_name().to_string(),
            default_enabled: true,
            is_primary_provider: false,
            uses_account_fallback: false,
            dashboard_url: None,
            subscription_dashboard_url: None,
            status_page_url: None,
            status_link_url: None,
        }
    }
}

// ============================================================================
// Provider Branding
// ============================================================================

/// Visual branding for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderBranding {
    /// Icon style to use.
    pub icon_style: IconStyle,
    /// Resource name for the icon asset.
    pub icon_resource_name: String,
    /// Primary color for this provider.
    pub color: ProviderColor,
}

impl ProviderBranding {
    /// Creates branding for a provider kind with defaults.
    pub fn for_provider(kind: ProviderKind) -> Self {
        let (icon_style, color) = match kind {
            ProviderKind::Codex => (IconStyle::Codex, ProviderColor::new(0.0, 0.64, 0.38)),
            ProviderKind::Claude => (IconStyle::Claude, ProviderColor::new(0.82, 0.58, 0.44)),
            ProviderKind::Cursor => (IconStyle::Cursor, ProviderColor::new(0.4, 0.4, 0.4)),
            ProviderKind::Gemini => (IconStyle::Gemini, ProviderColor::new(0.26, 0.52, 0.96)),
            ProviderKind::Copilot => (IconStyle::Copilot, ProviderColor::new(0.0, 0.47, 0.84)),
            ProviderKind::Factory => (IconStyle::Factory, ProviderColor::new(1.0, 0.6, 0.0)),
            ProviderKind::VertexAI => (IconStyle::VertexAI, ProviderColor::new(0.26, 0.52, 0.96)),
            ProviderKind::Zai => (IconStyle::Zai, ProviderColor::new(0.5, 0.0, 1.0)),
            ProviderKind::Augment => (IconStyle::Augment, ProviderColor::new(0.6, 0.2, 0.8)),
            ProviderKind::Kiro => (IconStyle::Kiro, ProviderColor::new(1.0, 0.4, 0.0)),
            ProviderKind::Antigravity => {
                (IconStyle::Antigravity, ProviderColor::new(0.2, 0.8, 0.8))
            }
            ProviderKind::MiniMax => (IconStyle::MiniMax, ProviderColor::new(0.9, 0.1, 0.3)),
        };

        Self {
            icon_style,
            icon_resource_name: format!("icon_{}", kind.cli_name()),
            color,
        }
    }
}

/// RGB color for provider branding.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProviderColor {
    /// Red component (0.0 - 1.0).
    pub red: f32,
    /// Green component (0.0 - 1.0).
    pub green: f32,
    /// Blue component (0.0 - 1.0).
    pub blue: f32,
}

impl ProviderColor {
    /// Creates a new color.
    pub const fn new(red: f32, green: f32, blue: f32) -> Self {
        Self { red, green, blue }
    }

    /// Converts to 8-bit RGB tuple.
    pub fn to_rgb8(&self) -> (u8, u8, u8) {
        (
            (self.red * 255.0) as u8,
            (self.green * 255.0) as u8,
            (self.blue * 255.0) as u8,
        )
    }

    /// Converts to hex string (e.g., "#FF6600").
    pub fn to_hex(&self) -> String {
        let (r, g, b) = self.to_rgb8();
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    }
}

impl Default for ProviderColor {
    fn default() -> Self {
        Self::new(0.5, 0.5, 0.5)
    }
}

/// Icon style for provider visual identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IconStyle {
    #[default]
    Codex,
    Claude,
    Cursor,
    Gemini,
    Copilot,
    Factory,
    VertexAI,
    Zai,
    Augment,
    Kiro,
    Antigravity,
    MiniMax,
    /// Combined/aggregate view icon.
    Combined,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_kind_display_name() {
        assert_eq!(ProviderKind::Claude.display_name(), "Claude");
        assert_eq!(ProviderKind::VertexAI.display_name(), "Vertex AI");
    }

    #[test]
    fn test_provider_kind_cli_name() {
        assert_eq!(ProviderKind::Claude.cli_name(), "claude");
        assert_eq!(ProviderKind::VertexAI.cli_name(), "vertexai");
    }

    #[test]
    fn test_provider_color_hex() {
        let color = ProviderColor::new(1.0, 0.5, 0.0);
        assert_eq!(color.to_hex(), "#FF7F00");
    }

    #[test]
    fn test_identity_display_string() {
        let mut identity = ProviderIdentity::new(ProviderKind::Claude);
        identity.account_email = Some("test@example.com".to_string());
        identity.account_organization = Some("Acme Inc".to_string());

        assert_eq!(identity.display_string(), "test@example.com (Acme Inc)");
    }
}
