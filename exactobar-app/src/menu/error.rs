//! Error display components with install hints and copy functionality.
//!
//! Provides enhanced error sections that show helpful install hints when
//! CLI tools are missing, plus one-click copy for error messages.

use exactobar_core::ProviderKind;
use gpui::prelude::FluentBuilder;
use gpui::*;
use tracing::info;

use crate::theme;

// ============================================================================
// Install Hint for Missing CLIs
// ============================================================================

/// Hint for installing a missing CLI tool.
#[derive(Debug, Clone)]
pub struct InstallHint {
    /// What's missing (e.g., "claude CLI")
    pub missing: String,
    /// Install command (e.g., "npm install -g @anthropic/claude-code")
    pub command: String,
}

/// Detects if an error indicates a missing CLI and returns install instructions.
pub fn get_install_hint(provider: ProviderKind, error: &str) -> Option<InstallHint> {
    let error_lower = error.to_lowercase();
    let missing_cli = error_lower.contains("not found")
        || error_lower.contains("command not found")
        || error_lower.contains("no such file")
        || error_lower.contains("cli not installed")
        || error_lower.contains("executable not found");

    if !missing_cli {
        return None;
    }

    let (missing, command) = match provider {
        ProviderKind::Codex => ("codex CLI", "npm install -g @openai/codex"),
        ProviderKind::Claude => ("claude CLI", "npm install -g @anthropic/claude-code"),
        ProviderKind::Cursor => ("Cursor app", "Download from cursor.com"),
        ProviderKind::Copilot => ("gh CLI", "brew install gh"),
        ProviderKind::Gemini => ("gcloud CLI", "brew install google-cloud-sdk"),
        ProviderKind::Kiro => ("kiro CLI", "npm install -g kiro-cli"),
        ProviderKind::Factory => ("factory CLI", "npm install -g @anthropic/factory"),
        ProviderKind::Zai => ("z CLI", "pip install z-cli"),
        ProviderKind::Augment => ("augment CLI", "brew install augment"),
        _ => return None,
    };

    Some(InstallHint {
        missing: missing.to_string(),
        command: command.to_string(),
    })
}

// ============================================================================
// Clipboard Helper
// ============================================================================

/// Copies text to the system clipboard.
pub fn copy_to_clipboard(text: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};

        if let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};

        // Try xclip first, then xsel
        if let Ok(mut child) = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};

        if let Ok(mut child) = Command::new("clip").stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
        }
    }
}

// ============================================================================
// Enhanced Error Section with Install Hints
// ============================================================================

pub struct EnhancedErrorSection {
    /// Short error summary (always visible)
    pub summary: String,
    /// Detailed error message (shown when expanded)
    #[allow(dead_code)]
    pub details: Option<String>,
    /// Install hint if CLI is missing
    pub install_hint: Option<InstallHint>,
}

/// Parse error message into summary (first line) and details (rest).
fn parse_error_message(error: &str) -> (String, Option<String>) {
    // If error contains newlines, first line is summary, rest is details
    if let Some(first_newline) = error.find('\n') {
        let summary = error[..first_newline].to_string();
        let details = error[first_newline + 1..].trim().to_string();
        if details.is_empty() {
            (summary, None)
        } else {
            (summary, Some(details))
        }
    } else {
        (error.to_string(), None)
    }
}

impl IntoElement for EnhancedErrorSection {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        // Parse the error to extract summary vs details from multi-line errors
        let (summary, parsed_details) = parse_error_message(&self.summary);

        // Combine parsed details with any explicit details
        let combined_details = match (parsed_details, &self.details) {
            (Some(parsed), Some(explicit)) => Some(format!("{}\n\n{}", parsed, explicit)),
            (Some(parsed), None) => Some(parsed),
            (None, Some(explicit)) => Some(explicit.clone()),
            (None, None) => None,
        };

        // Full error for copying includes everything
        let full_error = format!(
            "{}{}",
            self.summary, // Use original summary (has all lines)
            self.details
                .as_ref()
                .map(|d| format!("\n\n{d}"))
                .unwrap_or_default()
        );
        let full_error_for_copy = full_error.clone();

        let mut section = div()
            .id("error-section")
            .px(px(14.))
            .py(px(10.))
            .bg(theme::card_background())
            .border_b_1()
            .border_color(theme::glass_separator())
            .flex()
            .flex_col()
            .gap(px(8.));

        // Error header with icon and summary (first line only)
        section = section.child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().text_sm().child("⚠️"))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::error())
                        .flex_1()
                        .child(summary),
                ),
        );

        // Show details in a scrollable box if present
        if let Some(detail_text) = combined_details {
            section = section.child(
                div()
                    .id("error-details-scroll")
                    .mt(px(4.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(hsla(0., 0., 0.1, 0.5))
                    .max_h(px(150.))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .text_xs()
                            .font_family("SF Mono, Menlo, monospace")
                            .text_color(theme::text_secondary())
                            .child(
                                // Render each line separately to preserve newlines
                                div().flex().flex_col().gap(px(2.)).children(
                                    detail_text
                                        .lines()
                                        .map(|line| div().child(line.to_string()))
                                        .collect::<Vec<_>>(),
                                ),
                            ),
                    ),
            );
        }

        // Copy error button
        section = section.child(
            div()
                .id("copy-error-btn")
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .text_xs()
                .text_color(theme::text_secondary())
                .bg(theme::surface())
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .active(|s| s.bg(theme::active()))
                .on_mouse_down(MouseButton::Left, move |_, _window, _cx| {
                    copy_to_clipboard(&full_error_for_copy);
                    info!("Error copied to clipboard");
                })
                .flex()
                .items_center()
                .gap(px(4.))
                .child("📋")
                .child("Copy Error"),
        );

        // Install hint panel (if CLI is missing)
        if let Some(hint) = self.install_hint {
            let cmd_for_copy = hint.command.clone();

            section = section.child(
                div()
                    .mt(px(4.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(hsla(0.12, 0.8, 0.35, 0.15)) // Warm yellowish hint background
                    .border_1()
                    .border_color(hsla(0.12, 0.6, 0.5, 0.3))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    // "CLI not found" header
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::warning())
                            .child(format!("💡 {} not found", hint.missing)),
                    )
                    // "Install with:" label
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_secondary())
                            .child("Install with:"),
                    )
                    // Clickable install command (copies on click)
                    .child(
                        div()
                            .id("copy-install-cmd")
                            .px(px(8.))
                            .py(px(6.))
                            .rounded(px(4.))
                            .bg(theme::surface())
                            .text_xs()
                            .text_color(theme::text_primary())
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::hover()))
                            .active(|s| s.bg(theme::active()))
                            .on_mouse_down(MouseButton::Left, move |_, _window, _cx| {
                                copy_to_clipboard(&cmd_for_copy);
                                info!("Install command copied to clipboard");
                            })
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().text_color(theme::muted()).child("$"))
                            .child(hint.command)
                            .child(div().ml_auto().text_color(theme::muted()).child("📋")),
                    ),
            );
        }

        section
    }
}
