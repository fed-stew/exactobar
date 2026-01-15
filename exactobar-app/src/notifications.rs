//! Session quota notifications.
//!
//! Alerts users when they're approaching provider quota limits.

use exactobar_core::{ProviderKind, UsageSnapshot};
use std::collections::HashMap;
use tracing::{debug, info};

// Notification thresholds
const WARNING_THRESHOLD: f64 = 80.0;  // Warn at 80% used
const CRITICAL_THRESHOLD: f64 = 95.0; // Critical at 95% used

/// Tracks notification state to avoid spamming
#[derive(Default)]
pub struct NotificationTracker {
    /// Last notified threshold per provider
    last_notified: HashMap<ProviderKind, NotificationLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationLevel {
    None,
    Warning,
    Critical,
}

impl Default for NotificationLevel {
    fn default() -> Self {
        NotificationLevel::None
    }
}

impl NotificationTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if we should notify for this snapshot
    /// Returns the notification level if we should notify, None otherwise
    pub fn should_notify(
        &mut self,
        provider: ProviderKind,
        snapshot: &UsageSnapshot,
    ) -> Option<NotificationLevel> {
        let used_percent = snapshot.primary.as_ref()?.used_percent;
        
        let current_level = if used_percent >= CRITICAL_THRESHOLD {
            NotificationLevel::Critical
        } else if used_percent >= WARNING_THRESHOLD {
            NotificationLevel::Warning
        } else {
            NotificationLevel::None
        };

        let last_level = self.last_notified.get(&provider).copied().unwrap_or_default();

        // Only notify if we've crossed into a higher threshold
        if current_level > last_level {
            self.last_notified.insert(provider, current_level);
            if current_level != NotificationLevel::None {
                return Some(current_level);
            }
        }

        // Reset tracking if usage dropped (quota reset)
        if current_level < last_level {
            self.last_notified.insert(provider, current_level);
        }

        None
    }

    /// Reset notification state for a provider (e.g., after quota reset)
    #[allow(dead_code)]
    pub fn reset(&mut self, provider: ProviderKind) {
        self.last_notified.remove(&provider);
    }

    /// Reset all notification state
    #[allow(dead_code)]
    pub fn reset_all(&mut self) {
        self.last_notified.clear();
    }
}

/// Send a system notification
pub fn send_quota_notification(provider: ProviderKind, level: NotificationLevel, used_percent: f64) {
    let provider_name = provider.display_name();
    
    let (title, body) = match level {
        NotificationLevel::Warning => (
            format!("{} Quota Warning", provider_name),
            format!("You've used {:.0}% of your {} quota.", used_percent, provider_name),
        ),
        NotificationLevel::Critical => (
            format!("{} Quota Critical!", provider_name),
            format!("You've used {:.0}% of your {} quota. Consider slowing down.", used_percent, provider_name),
        ),
        NotificationLevel::None => return,
    };

    info!(
        provider = ?provider,
        level = ?level,
        percent = used_percent,
        "Sending quota notification"
    );

    // Use the system notification API
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // Use osascript for simple notifications
        // Escape quotes in body/title to avoid AppleScript injection
        let escaped_body = body.replace('"', "\\\"")
            .replace('\n', " ");
        let escaped_title = title.replace('"', "\\\"");
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escaped_body,
            escaped_title
        );
        
        let _ = Command::new("osascript")
            .args(["-e", &script])
            .spawn();
    }
    
    debug!("Notification sent: {} - {}", title, body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use exactobar_core::UsageWindow;

    fn make_snapshot(used_percent: f64) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(used_percent));
        snapshot
    }

    #[test]
    fn test_warning_notification() {
        let mut tracker = NotificationTracker::new();
        
        // Below warning - no notification
        let snap = make_snapshot(50.0);
        assert!(tracker.should_notify(ProviderKind::Claude, &snap).is_none());
        
        // At warning threshold - should notify
        let snap = make_snapshot(85.0);
        assert_eq!(
            tracker.should_notify(ProviderKind::Claude, &snap),
            Some(NotificationLevel::Warning)
        );
        
        // Still at warning - no duplicate
        let snap = make_snapshot(87.0);
        assert!(tracker.should_notify(ProviderKind::Claude, &snap).is_none());
    }

    #[test]
    fn test_critical_notification() {
        let mut tracker = NotificationTracker::new();
        
        // Jump straight to critical
        let snap = make_snapshot(96.0);
        assert_eq!(
            tracker.should_notify(ProviderKind::Claude, &snap),
            Some(NotificationLevel::Critical)
        );
    }

    #[test]
    fn test_reset_after_quota_refresh() {
        let mut tracker = NotificationTracker::new();
        
        // Hit critical
        let snap = make_snapshot(96.0);
        assert!(tracker.should_notify(ProviderKind::Claude, &snap).is_some());
        
        // Quota reset - usage drops
        let snap = make_snapshot(10.0);
        assert!(tracker.should_notify(ProviderKind::Claude, &snap).is_none());
        
        // Back to warning - should notify again
        let snap = make_snapshot(85.0);
        assert_eq!(
            tracker.should_notify(ProviderKind::Claude, &snap),
            Some(NotificationLevel::Warning)
        );
    }
}
