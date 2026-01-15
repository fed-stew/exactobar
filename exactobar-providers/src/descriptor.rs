//! Provider descriptor system.
//!
//! A descriptor contains all the static configuration for a provider:
//! - Metadata (display name, labels, URLs)
//! - Branding (colors, icons)
//! - Token cost configuration
//! - Fetch plan (how to get usage data)
//! - CLI configuration

use exactobar_core::{ProviderBranding, ProviderKind, ProviderMetadata};
use exactobar_fetch::{FetchContext, FetchPipeline, SourceMode};
use std::path::PathBuf;

// ============================================================================
// Provider Descriptor
// ============================================================================

/// Complete descriptor for a provider.
///
/// This contains all the static configuration needed to work with a provider,
/// including metadata, branding, and the fetch plan.
pub struct ProviderDescriptor {
    /// Provider identifier.
    pub id: ProviderKind,
    /// Display metadata.
    pub metadata: ProviderMetadata,
    /// Visual branding.
    pub branding: ProviderBranding,
    /// Token cost tracking configuration.
    pub token_cost: TokenCostConfig,
    /// How to fetch usage data.
    pub fetch_plan: FetchPlan,
    /// CLI tool configuration.
    pub cli: CliConfig,
}

impl ProviderDescriptor {
    /// Creates a new descriptor builder.
    pub fn builder(id: ProviderKind) -> ProviderDescriptorBuilder {
        ProviderDescriptorBuilder::new(id)
    }

    /// Returns the display name.
    pub fn display_name(&self) -> &str {
        &self.metadata.display_name
    }

    /// Returns the CLI name.
    pub fn cli_name(&self) -> &str {
        &self.cli.name
    }

    /// Builds the fetch pipeline for this provider.
    pub fn build_pipeline(&self, ctx: &FetchContext) -> FetchPipeline {
        (self.fetch_plan.build_pipeline)(ctx)
    }
}

// ============================================================================
// Token Cost Config
// ============================================================================

/// Configuration for token cost tracking.
pub struct TokenCostConfig {
    /// Whether this provider supports token cost tracking.
    pub supports_token_cost: bool,
    /// Function to get the log directory for this provider.
    pub log_directory: Option<fn() -> Option<PathBuf>>,
}

impl Default for TokenCostConfig {
    fn default() -> Self {
        Self {
            supports_token_cost: false,
            log_directory: None,
        }
    }
}

// ============================================================================
// Fetch Plan
// ============================================================================

/// Configuration for how to fetch usage data.
pub struct FetchPlan {
    /// Supported source modes in priority order.
    pub source_modes: Vec<SourceMode>,
    /// Function to build the fetch pipeline.
    pub build_pipeline: fn(&FetchContext) -> FetchPipeline,
}

impl Default for FetchPlan {
    fn default() -> Self {
        Self {
            source_modes: vec![SourceMode::Auto],
            build_pipeline: |_| FetchPipeline::new(),
        }
    }
}

// ============================================================================
// CLI Config
// ============================================================================

/// Configuration for CLI tool integration.
pub struct CliConfig {
    /// Primary CLI command name.
    pub name: &'static str,
    /// Alternative names/aliases.
    pub aliases: &'static [&'static str],
    /// Arguments to check CLI version.
    pub version_args: &'static [&'static str],
    /// Arguments to get usage data.
    pub usage_args: &'static [&'static str],
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            name: "",
            aliases: &[],
            version_args: &["--version"],
            usage_args: &[],
        }
    }
}

// ============================================================================
// Builder
// ============================================================================

/// Builder for ProviderDescriptor.
pub struct ProviderDescriptorBuilder {
    id: ProviderKind,
    metadata: Option<ProviderMetadata>,
    branding: Option<ProviderBranding>,
    token_cost: TokenCostConfig,
    fetch_plan: FetchPlan,
    cli: CliConfig,
}

impl ProviderDescriptorBuilder {
    /// Creates a new builder for the given provider.
    pub fn new(id: ProviderKind) -> Self {
        Self {
            id,
            metadata: None,
            branding: None,
            token_cost: TokenCostConfig::default(),
            fetch_plan: FetchPlan::default(),
            cli: CliConfig::default(),
        }
    }

    /// Sets the metadata.
    pub fn metadata(mut self, metadata: ProviderMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Sets the branding.
    pub fn branding(mut self, branding: ProviderBranding) -> Self {
        self.branding = Some(branding);
        self
    }

    /// Sets the token cost configuration.
    pub fn token_cost(mut self, config: TokenCostConfig) -> Self {
        self.token_cost = config;
        self
    }

    /// Sets the fetch plan.
    pub fn fetch_plan(mut self, plan: FetchPlan) -> Self {
        self.fetch_plan = plan;
        self
    }

    /// Sets the CLI configuration.
    pub fn cli(mut self, cli: CliConfig) -> Self {
        self.cli = cli;
        self
    }

    /// Builds the descriptor.
    pub fn build(self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id,
            metadata: self.metadata.unwrap_or_else(|| ProviderMetadata::for_provider(self.id)),
            branding: self.branding.unwrap_or_else(|| ProviderBranding::for_provider(self.id)),
            token_cost: self.token_cost,
            fetch_plan: self.fetch_plan,
            cli: self.cli,
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates default metadata for a provider with common fields.
pub fn default_metadata(id: ProviderKind) -> ProviderMetadata {
    ProviderMetadata::for_provider(id)
}

/// Creates default branding for a provider.
pub fn default_branding(id: ProviderKind) -> ProviderBranding {
    ProviderBranding::for_provider(id)
}
