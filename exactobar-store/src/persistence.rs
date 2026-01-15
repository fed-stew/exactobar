//! File persistence helpers.
//!
//! Handles loading and saving state to disk with proper security.

use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::error::StoreError;

// ============================================================================
// Default Paths
// ============================================================================

/// Returns the default configuration directory.
///
/// - macOS: `~/Library/Application Support/ExactoBar`
/// - Linux: `~/.config/exactobar`
/// - Windows: `%APPDATA%\ExactoBar`
pub fn default_config_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|h| h.join("Library").join("Application Support").join("ExactoBar"))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    #[cfg(not(target_os = "macos"))]
    {
        dirs::config_dir()
            .map(|c| c.join("exactobar"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Returns the default cache directory.
///
/// - macOS: `~/Library/Caches/ExactoBar`
/// - Linux: `~/.cache/exactobar`
/// - Windows: `%LOCALAPPDATA%\ExactoBar\cache`
pub fn default_cache_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|h| h.join("Library").join("Caches").join("ExactoBar"))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    #[cfg(not(target_os = "macos"))]
    {
        dirs::cache_dir()
            .map(|c| c.join("exactobar"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Returns the default settings file path.
pub fn default_settings_path() -> PathBuf {
    default_config_dir().join("settings.json")
}

/// Returns the default usage cache file path.
pub fn default_cache_path() -> PathBuf {
    default_cache_dir().join("usage_cache.json")
}

// ============================================================================
// Security: File Permissions
// ============================================================================

/// Sets restrictive file permissions (0o600) on Unix systems.
///
/// This ensures config files containing sensitive data are only
/// readable by the owner.
#[cfg(unix)]
async fn set_restrictive_permissions(path: &Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    
    let metadata = tokio::fs::metadata(path).await?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o600); // Owner read/write only
    tokio::fs::set_permissions(path, perms).await?;
    
    debug!(path = %path.display(), mode = "0600", "Set restrictive permissions");
    Ok(())
}

/// Sets restrictive directory permissions (0o700) on Unix systems.
///
/// This ensures config directories are only accessible by the owner.
#[cfg(unix)]
async fn set_restrictive_dir_permissions(path: &Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    
    let metadata = tokio::fs::metadata(path).await?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o700); // Owner read/write/execute only
    tokio::fs::set_permissions(path, perms).await?;
    
    debug!(path = %path.display(), mode = "0700", "Set restrictive directory permissions");
    Ok(())
}

/// No-op for non-Unix systems.
#[cfg(not(unix))]
async fn set_restrictive_permissions(_path: &Path) -> Result<(), StoreError> {
    Ok(())
}

/// No-op for non-Unix systems.
#[cfg(not(unix))]
async fn set_restrictive_dir_permissions(_path: &Path) -> Result<(), StoreError> {
    Ok(())
}

// ============================================================================
// File Operations
// ============================================================================

/// Creates parent directories with restrictive permissions.
///
/// On Unix systems, directories are created with 0o700 permissions
/// to ensure only the owner can access config files.
async fn create_secure_parent_dirs(path: &Path) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            debug!(path = %parent.display(), "Creating secure directory");
            tokio::fs::create_dir_all(parent).await?;
            
            // Set restrictive permissions on all created directories
            let mut current = parent.to_path_buf();
            while current.starts_with(default_config_dir()) || current.starts_with(default_cache_dir()) {
                if current.exists() {
                    set_restrictive_dir_permissions(&current).await?;
                }
                if !current.pop() {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Saves data to a JSON file with secure permissions.
///
/// Creates parent directories if they don't exist, writes atomically
/// (via temp file + rename), and sets restrictive permissions on Unix.
pub async fn save_json<T: Serialize>(path: &Path, data: &T) -> Result<(), StoreError> {
    debug!(path = %path.display(), "Saving JSON file");

    // Create parent directories with secure permissions
    create_secure_parent_dirs(path).await?;

    // Serialize to pretty JSON
    let json = serde_json::to_string_pretty(data)?;

    // Write atomically (write to temp file, then rename)
    let temp_path = path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, &json).await?;
    tokio::fs::rename(&temp_path, path).await?;

    // Set restrictive file permissions (Unix only)
    set_restrictive_permissions(path).await?;

    debug!(path = %path.display(), "JSON file saved securely");
    Ok(())
}

/// Loads data from a JSON file.
pub async fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, StoreError> {
    debug!(path = %path.display(), "Loading JSON file");

    let content = tokio::fs::read_to_string(path).await?;
    let data = serde_json::from_str(&content)?;

    debug!(path = %path.display(), "JSON file loaded");
    Ok(data)
}

/// Loads data from a JSON file, returning default if not found.
pub async fn load_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> T {
    match load_json(path).await {
        Ok(data) => data,
        Err(e) => {
            if !matches!(e, StoreError::Io(_)) {
                warn!(path = %path.display(), error = %e, "Failed to load, using defaults");
            }
            T::default()
        }
    }
}

/// Ensures a directory exists with secure permissions.
pub async fn ensure_dir(path: &Path) -> Result<(), StoreError> {
    if !path.exists() {
        debug!(path = %path.display(), "Creating directory");
        tokio::fs::create_dir_all(path).await?;
        set_restrictive_dir_permissions(path).await?;
    }
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_dir() {
        let path = default_config_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_default_cache_dir() {
        let path = default_cache_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_default_settings_path() {
        let path = default_settings_path();
        assert!(path.ends_with("settings.json"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.json");
        
        // Write a test file
        tokio::fs::write(&test_file, "{}").await.unwrap();
        
        // Set restrictive permissions
        set_restrictive_permissions(&test_file).await.unwrap();
        
        // Verify permissions
        let metadata = tokio::fs::metadata(&test_file).await.unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "File should have 0600 permissions");
    }
}
