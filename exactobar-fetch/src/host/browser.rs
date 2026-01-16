//! Browser cookie import for web scraping strategies.
//!
//! This module provides utilities for importing cookies from browsers,
//! which enables web scraping strategies that need authenticated sessions.
//!
//! ## Supported Browsers
//!
//! - **Firefox**: Full support (SQLite, no encryption)
//! - **Safari**: Full support on macOS (SQLite)
//! - **Chrome/Chromium**: Partial support (encrypted cookies require keychain access)
//! - **Arc**: Same as Chrome (Chromium-based)
//! - **Brave**: Same as Chrome (Chromium-based)
//! - **Edge**: Same as Chrome (Chromium-based)
//!
//! ## Security Note
//!
//! Cookie data is sensitive. This module only reads cookies for specific
//! domains requested by provider implementations.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, instrument, trace, warn};

use crate::error::BrowserError;

// ============================================================================
// Browser Enum
// ============================================================================

/// Supported browsers for cookie import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Browser {
    /// Apple Safari browser (macOS only).
    Safari,
    /// Google Chrome browser.
    Chrome,
    /// Mozilla Firefox browser.
    Firefox,
    /// Microsoft Edge browser.
    Edge,
    /// Arc browser (Chromium-based).
    Arc,
    /// Brave browser (Chromium-based).
    Brave,
}

impl Browser {
    /// Returns the display name for this browser.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Safari => "Safari",
            Self::Chrome => "Chrome",
            Self::Firefox => "Firefox",
            Self::Edge => "Edge",
            Self::Arc => "Arc",
            Self::Brave => "Brave",
        }
    }

    /// Returns the cookie database path for this browser on macOS.
    #[cfg(target_os = "macos")]
    pub fn cookie_db_path(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;

        let path = match self {
            // Safari uses Cookies.binarycookies or the newer SQLite format
            Self::Safari => {
                // Try the newer SQLite format first
                let sqlite_path = home.join("Library/Cookies/Cookies.sqlite");
                if sqlite_path.exists() {
                    sqlite_path
                } else {
                    // Fall back to binary format (we'll handle this separately)
                    home.join("Library/Cookies/Cookies.binarycookies")
                }
            }
            Self::Chrome => home.join(
                "Library/Application Support/Google/Chrome/Default/Cookies",
            ),
            Self::Firefox => {
                let profiles_dir = home.join("Library/Application Support/Firefox/Profiles");
                find_firefox_default_profile(&profiles_dir)?.join("cookies.sqlite")
            }
            Self::Edge => home.join(
                "Library/Application Support/Microsoft Edge/Default/Cookies",
            ),
            Self::Arc => home.join(
                "Library/Application Support/Arc/User Data/Default/Cookies",
            ),
            Self::Brave => home.join(
                "Library/Application Support/BraveSoftware/Brave-Browser/Default/Cookies",
            ),
        };

        Some(path)
    }

    /// Returns the cookie database path for this browser on Linux.
    #[cfg(target_os = "linux")]
    pub fn cookie_db_path(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;

        let path = match self {
            Self::Safari => return None,
            Self::Chrome => home.join(".config/google-chrome/Default/Cookies"),
            Self::Firefox => {
                let profiles_dir = home.join(".mozilla/firefox");
                find_firefox_default_profile(&profiles_dir)?.join("cookies.sqlite")
            }
            Self::Edge => home.join(".config/microsoft-edge/Default/Cookies"),
            Self::Arc => return None,
            Self::Brave => home.join(".config/BraveSoftware/Brave-Browser/Default/Cookies"),
        };

        Some(path)
    }

    /// Check if this browser is installed.
    pub fn is_installed(&self) -> bool {
        self.cookie_db_path().is_some_and(|p| p.exists())
    }

    /// Whether this browser uses encrypted cookies.
    pub fn uses_encrypted_cookies(&self) -> bool {
        matches!(
            self,
            Self::Chrome | Self::Edge | Self::Arc | Self::Brave
        )
    }

    /// Returns all browser variants.
    pub fn all() -> &'static [Browser] {
        &[
            Self::Safari,
            Self::Chrome,
            Self::Firefox,
            Self::Edge,
            Self::Arc,
            Self::Brave,
        ]
    }

    /// Default priority order for auto-detection.
    /// Safari and Firefox first (no encryption/keychain prompts).
    pub fn default_priority() -> &'static [Browser] {
        &[
            Self::Firefox, // No encryption, most reliable
            Self::Safari,  // No encryption on macOS
            Self::Chrome,  // Encrypted but common
            Self::Arc,
            Self::Brave,
            Self::Edge,
        ]
    }
}

/// Find the default Firefox profile directory.
fn find_firefox_default_profile(profiles_dir: &PathBuf) -> Option<PathBuf> {
    if !profiles_dir.exists() {
        return None;
    }

    let entries = std::fs::read_dir(profiles_dir).ok()?;

    // Look for a profile ending in ".default-release" or ".default"
    let mut default_profile: Option<PathBuf> = None;
    let mut any_profile: Option<PathBuf> = None;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() {
            if name.ends_with(".default-release") {
                return Some(entry.path());
            } else if name.ends_with(".default") {
                default_profile = Some(entry.path());
            } else {
                any_profile = Some(entry.path());
            }
        }
    }

    default_profile.or(any_profile)
}

// ============================================================================
// Cookie
// ============================================================================

/// A browser cookie.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Domain the cookie belongs to.
    pub domain: String,
    /// Path the cookie is valid for.
    pub path: String,
    /// Expiration time.
    pub expires: Option<DateTime<Utc>>,
    /// Whether the cookie requires HTTPS.
    pub secure: bool,
    /// Whether the cookie is HTTP-only.
    pub http_only: bool,
}

impl Cookie {
    /// Returns true if the cookie is expired.
    pub fn is_expired(&self) -> bool {
        self.expires.is_some_and(|exp| exp < Utc::now())
    }

    /// Returns true if this cookie matches the given domain.
    pub fn matches_domain(&self, domain: &str) -> bool {
        let cookie_domain = self.domain.trim_start_matches('.');
        domain == cookie_domain
            || domain.ends_with(&format!(".{}", cookie_domain))
            || cookie_domain.ends_with(&format!(".{}", domain))
    }
}

// ============================================================================
// Browser Cookie Importer
// ============================================================================

/// API for importing cookies from browsers.
#[derive(Debug, Clone, Default)]
pub struct BrowserCookieImporter;

impl BrowserCookieImporter {
    /// Creates a new browser cookie importer.
    pub fn new() -> Self {
        Self
    }

    /// Import cookies for a specific domain from a browser.
    #[instrument(skip(self), fields(browser = %browser.display_name(), domain = %domain))]
    pub async fn import_cookies(
        &self,
        browser: Browser,
        domain: &str,
    ) -> Result<Vec<Cookie>, BrowserError> {
        debug!("Importing cookies from browser");

        let db_path = browser.cookie_db_path().ok_or_else(|| {
            BrowserError::BrowserNotFound(browser.display_name().to_string())
        })?;

        if !db_path.exists() {
            return Err(BrowserError::DatabaseNotFound {
                browser: browser.display_name().to_string(),
                path: db_path.display().to_string(),
            });
        }

        // Different browsers use different formats
        let cookies = match browser {
            Browser::Safari => self.read_safari_cookies(&db_path, domain)?,
            Browser::Firefox => self.read_firefox_cookies(&db_path, domain)?,
            Browser::Chrome | Browser::Edge | Browser::Arc | Browser::Brave => {
                self.read_chromium_cookies(&db_path, domain, browser)?
            }
        };

        // Filter out expired cookies
        let cookies: Vec<Cookie> = cookies.into_iter().filter(|c| !c.is_expired()).collect();

        if cookies.is_empty() {
            return Err(BrowserError::NoCookiesFound(domain.to_string()));
        }

        debug!(count = cookies.len(), "Cookies imported successfully");
        Ok(cookies)
    }

    /// Import cookies from the first available browser (in priority order).
    #[instrument(skip(self, priority), fields(domain = %domain))]
    pub async fn import_cookies_auto(
        &self,
        domain: &str,
        priority: &[Browser],
    ) -> Result<(Browser, Vec<Cookie>), BrowserError> {
        debug!("Auto-importing cookies");

        let mut last_error = None;

        for browser in priority {
            match self.import_cookies(*browser, domain).await {
                Ok(cookies) => {
                    debug!(browser = %browser.display_name(), count = cookies.len(), "Found cookies");
                    return Ok((*browser, cookies));
                }
                Err(e) => {
                    trace!(browser = %browser.display_name(), error = %e, "Browser skipped");
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or(BrowserError::NoBrowsersAvailable))
    }

    /// Check which browsers are available.
    pub fn available_browsers(&self) -> Vec<Browser> {
        Browser::all()
            .iter()
            .filter(|b| b.is_installed())
            .copied()
            .collect()
    }

    /// Build a cookie header string for HTTP requests.
    pub fn cookies_to_header(cookies: &[Cookie]) -> String {
        cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    }

    // ========================================================================
    // Safari Cookies
    // ========================================================================

    /// Read Safari cookies from SQLite database.
    fn read_safari_cookies(
        &self,
        db_path: &PathBuf,
        domain: &str,
    ) -> Result<Vec<Cookie>, BrowserError> {
        debug!(path = %db_path.display(), "Reading Safari cookies");

        // Safari uses binarycookies format on older systems, SQLite on newer
        if db_path.extension().and_then(|e| e.to_str()) == Some("binarycookies") {
            return self.read_safari_binary_cookies(db_path, domain);
        }

        // SQLite format - need to copy to temp because Safari locks the file
        let temp_path = copy_to_temp(db_path)?;

        let conn = Connection::open_with_flags(&temp_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| BrowserError::ReadFailed(format!("SQLite open error: {}", e)))?;

        // Safari SQLite schema:
        // CREATE TABLE cookies (id INTEGER PRIMARY KEY, name TEXT, value TEXT,
        //   domain TEXT, path TEXT, expires REAL, creation REAL, ...)
        let mut stmt = conn
            .prepare(
                "SELECT name, value, domain, path, expires, is_secure, is_httponly
                 FROM cookies
                 WHERE domain LIKE ?1 OR domain LIKE ?2",
            )
            .map_err(|e| BrowserError::ReadFailed(format!("Prepare error: {}", e)))?;

        let domain_pattern = format!("%{}", domain);
        let exact_domain = domain.to_string();

        let cookies = stmt
            .query_map([&domain_pattern, &exact_domain], |row| {
                let expires_raw: Option<f64> = row.get(4).ok();
                let expires = expires_raw.and_then(|ts| {
                    // Safari uses Mac absolute time (seconds since 2001-01-01)
                    let unix_ts = ts + 978307200.0; // Convert to Unix timestamp
                    Utc.timestamp_opt(unix_ts as i64, 0).single()
                });

                Ok(Cookie {
                    name: row.get(0)?,
                    value: row.get(1)?,
                    domain: row.get(2)?,
                    path: row.get(3)?,
                    expires,
                    secure: row.get::<_, i32>(5).unwrap_or(0) != 0,
                    http_only: row.get::<_, i32>(6).unwrap_or(0) != 0,
                })
            })
            .map_err(|e| BrowserError::ReadFailed(format!("Query error: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        // Clean up temp file
        let _ = fs::remove_file(&temp_path);

        Ok(cookies)
    }

    /// Read Safari binary cookies format.
    fn read_safari_binary_cookies(
        &self,
        _db_path: &PathBuf,
        _domain: &str,
    ) -> Result<Vec<Cookie>, BrowserError> {
        // Binary format is complex - for now, return error
        // In production, we'd implement the binary parser
        warn!("Safari binary cookie format not supported - please upgrade macOS");
        Err(BrowserError::ReadFailed(
            "Safari binary cookie format requires macOS 10.15+".to_string(),
        ))
    }

    // ========================================================================
    // Firefox Cookies
    // ========================================================================

    /// Read Firefox cookies from SQLite database.
    fn read_firefox_cookies(
        &self,
        db_path: &PathBuf,
        domain: &str,
    ) -> Result<Vec<Cookie>, BrowserError> {
        debug!(path = %db_path.display(), "Reading Firefox cookies");

        // Firefox locks the database, so copy to temp
        let temp_path = copy_to_temp(db_path)?;

        let conn = Connection::open_with_flags(&temp_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| BrowserError::ReadFailed(format!("SQLite open error: {}", e)))?;

        // Firefox schema:
        // CREATE TABLE moz_cookies (id INTEGER PRIMARY KEY, baseDomain TEXT,
        //   name TEXT, value TEXT, host TEXT, path TEXT, expiry INTEGER,
        //   isSecure INTEGER, isHttpOnly INTEGER, ...)
        let mut stmt = conn
            .prepare(
                "SELECT name, value, host, path, expiry, isSecure, isHttpOnly
                 FROM moz_cookies
                 WHERE host LIKE ?1 OR baseDomain LIKE ?2",
            )
            .map_err(|e| BrowserError::ReadFailed(format!("Prepare error: {}", e)))?;

        let domain_pattern = format!("%{}", domain);

        let cookies = stmt
            .query_map([&domain_pattern, &domain.to_string()], |row| {
                let expiry: i64 = row.get(4)?;
                let expires = if expiry > 0 {
                    Utc.timestamp_opt(expiry, 0).single()
                } else {
                    None
                };

                Ok(Cookie {
                    name: row.get(0)?,
                    value: row.get(1)?,
                    domain: row.get(2)?,
                    path: row.get(3)?,
                    expires,
                    secure: row.get::<_, i32>(5)? != 0,
                    http_only: row.get::<_, i32>(6)? != 0,
                })
            })
            .map_err(|e| BrowserError::ReadFailed(format!("Query error: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        // Clean up temp file
        let _ = fs::remove_file(&temp_path);

        Ok(cookies)
    }

    // ========================================================================
    // Chromium Cookies (Chrome, Edge, Arc, Brave)
    // ========================================================================

    /// Read Chromium-based browser cookies.
    fn read_chromium_cookies(
        &self,
        db_path: &PathBuf,
        domain: &str,
        browser: Browser,
    ) -> Result<Vec<Cookie>, BrowserError> {
        debug!(path = %db_path.display(), browser = %browser.display_name(), "Reading Chromium cookies");

        // Chromium locks the database, so copy to temp
        let temp_path = copy_to_temp(db_path)?;

        let conn = Connection::open_with_flags(&temp_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| BrowserError::ReadFailed(format!("SQLite open error: {}", e)))?;

        // Chromium schema:
        // CREATE TABLE cookies (creation_utc INTEGER, host_key TEXT, top_frame_site_key TEXT,
        //   name TEXT, value TEXT, encrypted_value BLOB, path TEXT, expires_utc INTEGER,
        //   is_secure INTEGER, is_httponly INTEGER, ...)
        let mut stmt = conn
            .prepare(
                "SELECT name, value, encrypted_value, host_key, path, expires_utc, is_secure, is_httponly
                 FROM cookies
                 WHERE host_key LIKE ?1 OR host_key = ?2",
            )
            .map_err(|e| BrowserError::ReadFailed(format!("Prepare error: {}", e)))?;

        let domain_pattern = format!("%{}", domain);
        let exact_domain = format!(".{}", domain);

        let cookies_result: Vec<_> = stmt
            .query_map([&domain_pattern, &exact_domain], |row| {
                let name: String = row.get(0)?;
                let value: String = row.get(1)?;
                let encrypted_value: Vec<u8> = row.get(2)?;
                let host_key: String = row.get(3)?;
                let path: String = row.get(4)?;
                let expires_utc: i64 = row.get(5)?;
                let is_secure: i32 = row.get(6)?;
                let is_httponly: i32 = row.get(7)?;

                Ok((name, value, encrypted_value, host_key, path, expires_utc, is_secure, is_httponly))
            })
            .map_err(|e| BrowserError::ReadFailed(format!("Query error: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        // Clean up temp file
        let _ = fs::remove_file(&temp_path);

        // Process cookies, attempting decryption if needed
        let mut cookies = Vec::new();

        for (name, value, encrypted_value, host_key, path, expires_utc, is_secure, is_httponly) in cookies_result {
            // Chromium stores expires as microseconds since Windows epoch (1601-01-01)
            let expires = if expires_utc > 0 {
                // Convert from Windows epoch microseconds to Unix timestamp
                let unix_micros = expires_utc - 11644473600000000; // Difference in microseconds
                let unix_secs = unix_micros / 1000000;
                Utc.timestamp_opt(unix_secs, 0).single()
            } else {
                None
            };

            // Try to get cookie value
            let cookie_value = if !value.is_empty() {
                // Unencrypted value (rare but possible)
                value
            } else if !encrypted_value.is_empty() {
                // Try to decrypt
                match decrypt_chromium_cookie(&encrypted_value, browser) {
                    Ok(decrypted) => decrypted,
                    Err(e) => {
                        trace!(name = %name, error = %e, "Failed to decrypt cookie, skipping");
                        continue;
                    }
                }
            } else {
                continue;
            };

            cookies.push(Cookie {
                name,
                value: cookie_value,
                domain: host_key,
                path,
                expires,
                secure: is_secure != 0,
                http_only: is_httponly != 0,
            });
        }

        Ok(cookies)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Copy a database file to a temp location to avoid locking issues.
fn copy_to_temp(source: &PathBuf) -> Result<PathBuf, BrowserError> {
    let temp_dir = std::env::temp_dir();
    let temp_name = format!(
        "exactobar_cookies_{}.sqlite",
        std::process::id()
    );
    let temp_path = temp_dir.join(temp_name);

    fs::copy(source, &temp_path).map_err(|e| {
        BrowserError::ReadFailed(format!("Failed to copy database: {}", e))
    })?;

    Ok(temp_path)
}

/// Decrypt a Chromium encrypted cookie value.
#[cfg(target_os = "macos")]
fn decrypt_chromium_cookie(encrypted: &[u8], browser: Browser) -> Result<String, BrowserError> {
    // Chromium encryption on macOS:
    // - First 3 bytes are "v10" or "v11" version marker
    // - Rest is AES-128-CBC encrypted with key from Keychain

    if encrypted.len() < 4 {
        return Err(BrowserError::DecryptionFailed("Data too short".to_string()));
    }

    // Check version marker
    let version = &encrypted[0..3];
    if version != b"v10" && version != b"v11" {
        return Err(BrowserError::DecryptionFailed(format!(
            "Unknown encryption version: {:?}",
            version
        )));
    }

    // Get the encryption key from Keychain
    let service_name = match browser {
        Browser::Chrome => "Chrome Safe Storage",
        Browser::Edge => "Microsoft Edge Safe Storage",
        Browser::Arc => "Arc Safe Storage",
        Browser::Brave => "Brave Safe Storage",
        _ => return Err(BrowserError::DecryptionFailed("Not a Chromium browser".to_string())),
    };

    // Try to get key from keychain using keyring crate
    let entry = keyring::Entry::new(service_name, "")
        .map_err(|e| BrowserError::DecryptionFailed(format!("Keychain error: {}", e)))?;

    let password = entry
        .get_password()
        .map_err(|e| BrowserError::DecryptionFailed(format!("No keychain entry: {}", e)))?;

    // Derive the actual encryption key using PBKDF2
    // Chrome uses: PBKDF2(password, salt="saltysalt", iterations=1003, dkLen=16)
    use std::num::NonZeroU32;
    let salt = b"saltysalt";
    let iterations = NonZeroU32::new(1003).unwrap();
    let mut key = [0u8; 16];

    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA1,
        iterations,
        salt,
        password.as_bytes(),
        &mut key,
    );

    // Decrypt using AES-128-CBC
    // IV is 16 bytes of spaces for Chrome
    let iv = [b' '; 16];
    let ciphertext = &encrypted[3..];

    let decrypted = decrypt_aes_cbc(&key, &iv, ciphertext)
        .map_err(|e| BrowserError::DecryptionFailed(format!("AES error: {}", e)))?;

    String::from_utf8(decrypted)
        .map_err(|e| BrowserError::DecryptionFailed(format!("UTF-8 error: {}", e)))
}

#[cfg(not(target_os = "macos"))]
fn decrypt_chromium_cookie(_encrypted: &[u8], _browser: Browser) -> Result<String, BrowserError> {
    // Linux uses libsecret, Windows uses DPAPI - not implemented yet
    Err(BrowserError::DecryptionFailed(
        "Chromium cookie decryption only supported on macOS".to_string(),
    ))
}

/// Decrypt data using AES-128-CBC.
///
/// # Security
/// Key material is passed via environment variables rather than CLI arguments
/// to prevent exposure in process listings (e.g., `ps aux`). Environment
/// variables are process-private and not visible to other users.
#[cfg(target_os = "macos")]
fn decrypt_aes_cbc(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    use std::process::Command;

    // SECURITY FIX: Pass key/IV via environment variables instead of CLI args.
    // CLI arguments are visible in process listings (`ps aux`), but environment
    // variables are process-private and not exposed to other users.
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("openssl enc -d -aes-128-cbc -K \"$OPENSSL_KEY\" -iv \"$OPENSSL_IV\"")
        .env("OPENSSL_KEY", hex::encode(key))
        .env("OPENSSL_IV", hex::encode(iv))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    use std::io::Write;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(data).map_err(|e| e.to_string())?;
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err("Decryption failed".to_string())
    }
}

// Hex encoding helper for key material (used via environment variables)
mod hex {
    #[allow(dead_code)]
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie_matches_domain() {
        let cookie = Cookie {
            name: "session".to_string(),
            value: "abc123".to_string(),
            domain: ".anthropic.com".to_string(),
            path: "/".to_string(),
            expires: None,
            secure: true,
            http_only: true,
        };

        assert!(cookie.matches_domain("anthropic.com"));
        assert!(cookie.matches_domain("console.anthropic.com"));
        assert!(cookie.matches_domain("api.anthropic.com"));
        assert!(!cookie.matches_domain("notanthropic.com"));
    }

    #[test]
    fn test_cookie_matches_domain_exact() {
        let cookie = Cookie {
            name: "session".to_string(),
            value: "abc123".to_string(),
            domain: "cursor.com".to_string(),
            path: "/".to_string(),
            expires: None,
            secure: true,
            http_only: true,
        };

        assert!(cookie.matches_domain("cursor.com"));
        assert!(cookie.matches_domain("www.cursor.com"));
    }

    #[test]
    fn test_cookies_to_header() {
        let cookies = vec![
            Cookie {
                name: "session".to_string(),
                value: "abc".to_string(),
                domain: "example.com".to_string(),
                path: "/".to_string(),
                expires: None,
                secure: true,
                http_only: true,
            },
            Cookie {
                name: "token".to_string(),
                value: "xyz".to_string(),
                domain: "example.com".to_string(),
                path: "/".to_string(),
                expires: None,
                secure: true,
                http_only: false,
            },
        ];

        let header = BrowserCookieImporter::cookies_to_header(&cookies);
        assert_eq!(header, "session=abc; token=xyz");
    }

    #[test]
    fn test_browser_display_name() {
        assert_eq!(Browser::Safari.display_name(), "Safari");
        assert_eq!(Browser::Chrome.display_name(), "Chrome");
        assert_eq!(Browser::Firefox.display_name(), "Firefox");
    }

    #[test]
    fn test_browser_uses_encrypted_cookies() {
        assert!(!Browser::Safari.uses_encrypted_cookies());
        assert!(!Browser::Firefox.uses_encrypted_cookies());
        assert!(Browser::Chrome.uses_encrypted_cookies());
        assert!(Browser::Arc.uses_encrypted_cookies());
        assert!(Browser::Brave.uses_encrypted_cookies());
    }

    #[test]
    fn test_available_browsers() {
        let importer = BrowserCookieImporter::new();
        let available = importer.available_browsers();
        println!("Available browsers: {:?}", available);
        // Don't assert anything - depends on system
    }

    #[test]
    fn test_default_priority() {
        let priority = Browser::default_priority();
        // Firefox should be first (no encryption)
        assert_eq!(priority[0], Browser::Firefox);
        assert_eq!(priority[1], Browser::Safari);
    }

    #[test]
    fn test_cookie_is_expired() {
        let past = Utc::now() - chrono::Duration::hours(1);
        let future = Utc::now() + chrono::Duration::hours(1);

        let expired_cookie = Cookie {
            name: "test".to_string(),
            value: "val".to_string(),
            domain: "example.com".to_string(),
            path: "/".to_string(),
            expires: Some(past),
            secure: false,
            http_only: false,
        };
        assert!(expired_cookie.is_expired());

        let valid_cookie = Cookie {
            name: "test".to_string(),
            value: "val".to_string(),
            domain: "example.com".to_string(),
            path: "/".to_string(),
            expires: Some(future),
            secure: false,
            http_only: false,
        };
        assert!(!valid_cookie.is_expired());

        let session_cookie = Cookie {
            name: "test".to_string(),
            value: "val".to_string(),
            domain: "example.com".to_string(),
            path: "/".to_string(),
            expires: None,
            secure: false,
            http_only: false,
        };
        assert!(!session_cookie.is_expired());
    }
}
