// Lint configuration for this crate
// TODO: Re-enable missing_docs once all providers are documented
#![allow(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::unused_self)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::doc_markdown)] // TODO: Fix doc backticks
#![allow(clippy::uninlined_format_args)] // TODO: Fix format! strings
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::redundant_else)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::manual_strip)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::duplicated_attributes)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::unnecessary_lazy_evaluations)]
#![allow(clippy::needless_continue)]
#![allow(clippy::let_and_return)]
#![allow(clippy::stable_sort_primitive)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::unnecessary_literal_bound)]

//! # `ExactoBar` Providers
//!
//! Provider-specific implementations for the `ExactoBar` application.
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
