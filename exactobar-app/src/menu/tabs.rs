//! Tab selection types for the menu panel.
//!
//! Provides the `SelectedTab` enum for switching between "All" providers view
//! and individual provider views.

use exactobar_core::ProviderKind;

/// Represents the currently selected tab in the menu panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectedTab {
    /// Show all enabled providers stacked vertically.
    All,
    /// Show a single provider's details.
    Provider(ProviderKind),
}

impl SelectedTab {
    /// Returns the display name for this tab.
    pub fn display_name(&self) -> &'static str {
        match self {
            SelectedTab::All => "All",
            SelectedTab::Provider(p) => p.display_name(),
        }
    }

    /// Returns true if this is the "All" tab.
    pub fn is_all(&self) -> bool {
        matches!(self, SelectedTab::All)
    }

    /// Returns the provider if this is a single-provider tab.
    pub fn provider(&self) -> Option<ProviderKind> {
        match self {
            SelectedTab::All => None,
            SelectedTab::Provider(p) => Some(*p),
        }
    }
}

impl Default for SelectedTab {
    fn default() -> Self {
        SelectedTab::All
    }
}
