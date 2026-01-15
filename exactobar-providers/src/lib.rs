// Lint configuration for this crate
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

//! # ExactoBar Providers
//!
//! Provider-specific implementations for the ExactoBar application.
//!
//! This crate contains the concrete implementations for each supported
//! LLM provider. Each provider module includes:
//!
//! - **Descriptor**: Static configuration (metadata, branding, fetch plan)
//! - **Strategies**: Fetch strategy implementations (CLI, OAuth, Web)
//! - **Parser**: Response parsing for various formats
//!
//! ## Supported Providers (12 total)
//!
//! | Provider | CLI | OAuth | API Key | Web | Local | Status |
//! |----------|-----|-------|---------|-----|-------|--------|
//! | Codex (OpenAI) | ✅ | ❌ | ✅ | 🔜 | ❌ | Primary |
//! | Claude (Anthropic) | ✅ | ✅ | ❌ | ✅ | ❌ | Primary |
//! | Cursor | ❌ | ❌ | ❌ | ✅ | ✅ | Active |
//! | Copilot (GitHub) | ❌ | ✅ | ✅ | ❌ | ❌ | Active |
//! | Gemini (Google) | ✅ | ✅ | ❌ | ❌ | ❌ | Active |
//! | VertexAI (GCP) | ❌ | ✅ | ❌ | ❌ | ✅ | Active |
//! | Factory (Droid) | ❌ | ❌ | ❌ | ✅ | ✅ | Active |
//! | z.ai | ❌ | ❌ | ✅ | ❌ | ❌ | Active |
//! | Augment | ❌ | ❌ | ❌ | ✅ | ❌ | Active |
//! | Kiro (AWS) | ✅ | ❌ | ❌ | ❌ | ❌ | Active |
//! | MiniMax | ❌ | ❌ | ❌ | ✅ | ✅ | Active |
//! | Antigravity | ❌ | ❌ | ❌ | ❌ | ✅ | Active |
//!
//! ## Usage
//!
//! ```ignore
//! use exactobar_providers::{ProviderRegistry};
//! use exactobar_core::ProviderKind;
//! use exactobar_fetch::FetchContext;
//!
//! // Get a provider by kind
//! let desc = ProviderRegistry::get(ProviderKind::Claude).unwrap();
//!
//! // Build and execute the fetch pipeline
//! let ctx = FetchContext::new();
//! let pipeline = desc.build_pipeline(&ctx);
//! let outcome = pipeline.execute(&ctx).await;
//! ```

pub mod descriptor;
pub mod registry;

// Provider modules (alphabetical)
pub mod antigravity;
pub mod augment;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod factory;
pub mod gemini;
pub mod kiro;
pub mod minimax;
pub mod vertexai;
pub mod zai;

// Re-export key types
pub use descriptor::{
    CliConfig, FetchPlan, ProviderDescriptor, ProviderDescriptorBuilder, TokenCostConfig,
};
pub use registry::ProviderRegistry;

// Re-export provider descriptors
pub use antigravity::antigravity_descriptor;
pub use augment::augment_descriptor;
pub use claude::claude_descriptor;
pub use codex::codex_descriptor;
pub use copilot::copilot_descriptor;
pub use cursor::cursor_descriptor;
pub use factory::factory_descriptor;
pub use gemini::gemini_descriptor;
pub use kiro::kiro_descriptor;
pub use minimax::minimax_descriptor;
pub use vertexai::vertexai_descriptor;
pub use zai::zai_descriptor;

// Re-export strategy types for convenience
pub use antigravity::AntigravityLocalStrategy;
pub use augment::AugmentWebStrategy;
pub use claude::{ClaudeCliStrategy, ClaudeOAuthStrategy, ClaudeWebStrategy};
pub use codex::{CodexApiStrategy, CodexCliStrategy};
pub use copilot::{CopilotApiStrategy, CopilotEnvStrategy};
pub use cursor::{CursorLocalStrategy, CursorWebStrategy};
pub use factory::{FactoryLocalStrategy, FactoryWebStrategy};
pub use gemini::{GeminiCliStrategy, GeminiOAuthStrategy};
pub use kiro::KiroCliStrategy;
pub use minimax::{MiniMaxLocalStrategy, MiniMaxWebStrategy};
pub use vertexai::{VertexAILocalStrategy, VertexAIOAuthStrategy};
pub use zai::ZaiApiStrategy;
#[cfg(test)]
mod parser_edge_tests;
