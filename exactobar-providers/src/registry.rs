//! Provider registry for managing all provider descriptors.
//!
//! The registry provides static access to all provider configurations
//! and is the central point for looking up providers.

use exactobar_core::ProviderKind;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::descriptor::ProviderDescriptor;
use crate::antigravity::antigravity_descriptor;
use crate::augment::augment_descriptor;
use crate::claude::claude_descriptor;
use crate::codex::codex_descriptor;
use crate::copilot::copilot_descriptor;
use crate::cursor::cursor_descriptor;
use crate::factory::factory_descriptor;
use crate::gemini::gemini_descriptor;
use crate::kiro::kiro_descriptor;
use crate::minimax::minimax_descriptor;
use crate::vertexai::vertexai_descriptor;
use crate::zai::zai_descriptor;

// ============================================================================
// Static Registry
// ============================================================================

/// Static storage for all provider descriptors.
static DESCRIPTORS: OnceLock<Vec<ProviderDescriptor>> = OnceLock::new();

/// Static storage for CLI name to provider kind mapping.
static CLI_NAME_MAP: OnceLock<HashMap<String, ProviderKind>> = OnceLock::new();

/// Initializes all provider descriptors.
///
/// Providers are ordered by priority/importance:
/// 1. Primary providers (Codex, Claude)
/// 2. Popular IDE providers (Cursor, Copilot)
/// 3. Cloud providers (Gemini, VertexAI)
/// 4. Other providers (Factory, Zai, Augment, Kiro, MiniMax, Antigravity)
fn init_descriptors() -> Vec<ProviderDescriptor> {
    vec![
        // Primary providers
        codex_descriptor(),
        claude_descriptor(),
        // IDE providers
        cursor_descriptor(),
        copilot_descriptor(),
        // Cloud providers
        gemini_descriptor(),
        vertexai_descriptor(),
        // Other providers
        factory_descriptor(),
        zai_descriptor(),
        augment_descriptor(),
        kiro_descriptor(),
        minimax_descriptor(),
        antigravity_descriptor(),
    ]
}

/// Builds the CLI name to provider kind mapping.
fn build_cli_name_map(descriptors: &[ProviderDescriptor]) -> HashMap<String, ProviderKind> {
    let mut map = HashMap::new();

    for desc in descriptors {
        // Primary CLI name
        map.insert(desc.cli.name.to_string(), desc.id);

        // Aliases
        for alias in desc.cli.aliases {
            map.insert((*alias).to_string(), desc.id);
        }
    }

    map
}

// ============================================================================
// Provider Registry
// ============================================================================

/// Global registry of all provider descriptors.
///
/// The registry is initialized lazily on first access and provides
/// thread-safe access to provider configurations.
pub struct ProviderRegistry;

impl ProviderRegistry {
    /// Returns all provider descriptors.
    pub fn all() -> &'static [ProviderDescriptor] {
        DESCRIPTORS.get_or_init(init_descriptors)
    }

    /// Gets a provider descriptor by kind.
    pub fn get(id: ProviderKind) -> Option<&'static ProviderDescriptor> {
        Self::all().iter().find(|d| d.id == id)
    }

    /// Returns the CLI name to provider kind mapping.
    pub fn cli_name_map() -> &'static HashMap<String, ProviderKind> {
        CLI_NAME_MAP.get_or_init(|| build_cli_name_map(Self::all()))
    }

    /// Looks up a provider by CLI name.
    pub fn get_by_cli_name(name: &str) -> Option<&'static ProviderDescriptor> {
        let kind = Self::cli_name_map().get(name)?;
        Self::get(*kind)
    }

    /// Returns all enabled-by-default providers.
    pub fn default_enabled() -> Vec<&'static ProviderDescriptor> {
        Self::all()
            .iter()
            .filter(|d| d.metadata.default_enabled)
            .collect()
    }

    /// Returns all primary providers.
    pub fn primary_providers() -> Vec<&'static ProviderDescriptor> {
        Self::all()
            .iter()
            .filter(|d| d.metadata.is_primary_provider)
            .collect()
    }

    /// Returns the number of registered providers.
    pub fn count() -> usize {
        Self::all().len()
    }

    /// Returns all provider kinds.
    pub fn kinds() -> Vec<ProviderKind> {
        Self::all().iter().map(|d| d.id).collect()
    }

    /// Returns providers that support the given source mode.
    pub fn with_source_mode(mode: exactobar_fetch::SourceMode) -> Vec<&'static ProviderDescriptor> {
        Self::all()
            .iter()
            .filter(|d| d.fetch_plan.source_modes.contains(&mode))
            .collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_all_12_providers() {
        let all = ProviderRegistry::all();
        assert_eq!(all.len(), 12, "Should have exactly 12 providers");
    }

    #[test]
    fn test_registry_get_all_kinds() {
        // Test that we can get each provider by its kind
        let kinds = [
            ProviderKind::Codex,
            ProviderKind::Claude,
            ProviderKind::Cursor,
            ProviderKind::Copilot,
            ProviderKind::Gemini,
            ProviderKind::VertexAI,
            ProviderKind::Factory,
            ProviderKind::Zai,
            ProviderKind::Augment,
            ProviderKind::Kiro,
            ProviderKind::MiniMax,
            ProviderKind::Antigravity,
        ];

        for kind in kinds {
            let desc = ProviderRegistry::get(kind);
            assert!(desc.is_some(), "Should find provider {:?}", kind);
            assert_eq!(desc.unwrap().id, kind);
        }
    }

    #[test]
    fn test_cli_name_lookup() {
        // Primary names
        assert!(ProviderRegistry::get_by_cli_name("codex").is_some());
        assert!(ProviderRegistry::get_by_cli_name("claude").is_some());
        assert!(ProviderRegistry::get_by_cli_name("cursor").is_some());

        // Aliases
        let openai = ProviderRegistry::get_by_cli_name("openai");
        assert!(openai.is_some());
        assert_eq!(openai.unwrap().id, ProviderKind::Codex);

        let gcloud = ProviderRegistry::get_by_cli_name("gcloud");
        assert!(gcloud.is_some());
        // gcloud maps to both Gemini and VertexAI - check it's one of them
    }

    #[test]
    fn test_default_enabled() {
        let enabled = ProviderRegistry::default_enabled();
        assert!(!enabled.is_empty());

        // Codex and Claude should be enabled by default
        let kinds: Vec<_> = enabled.iter().map(|d| d.id).collect();
        assert!(kinds.contains(&ProviderKind::Codex));
        assert!(kinds.contains(&ProviderKind::Claude));

        // Cursor should NOT be enabled by default
        assert!(!kinds.contains(&ProviderKind::Cursor));
    }

    #[test]
    fn test_primary_providers() {
        let primary = ProviderRegistry::primary_providers();

        // Codex and Claude should be primary
        let kinds: Vec<_> = primary.iter().map(|d| d.id).collect();
        assert!(kinds.contains(&ProviderKind::Codex));
        assert!(kinds.contains(&ProviderKind::Claude));

        // Cursor should NOT be primary
        assert!(!kinds.contains(&ProviderKind::Cursor));
    }

    #[test]
    fn test_provider_count() {
        assert_eq!(ProviderRegistry::count(), 12);
    }

    #[test]
    fn test_all_kinds_returned() {
        let kinds = ProviderRegistry::kinds();
        assert_eq!(kinds.len(), 12);
    }
}
