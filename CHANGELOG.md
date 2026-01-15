# Changelog

All notable changes to ExactoBar will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of ExactoBar
- Support for 12 LLM providers: Claude, Codex, Cursor, Copilot, Gemini, VertexAI, Factory, z.ai, Augment, Kiro, Antigravity, MiniMax
- macOS menu bar application using GPUI framework
- CLI tool for terminal-based usage monitoring
- Multiple authentication strategies: CLI, OAuth, Web scraping, Local database
- Secure credential storage via system keychain
- Token cost tracking from local logs
- Watch mode for live updates

### Security
- Credentials stored in system keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- TLS 1.3 via rustls for all network requests
- Domain allowlist for SSRF protection
- Parameterized SQL queries for SQLite operations
- File permissions set to 0o600 for sensitive files

## [0.1.0] - Unreleased

Initial release.
