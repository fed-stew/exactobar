# ExactoBar 🎯

> A precision tool for monitoring your LLM provider usage - right from your macOS menu bar.

ExactoBar is a pure Rust port of [CodexBar](https://github.com/example/codexbar), rebuilt from the ground up using the [GPUI](https://github.com/zed-industries/zed) framework for a native macOS experience.

## What is ExactoBar?

ExactoBar sits in your menu bar and monitors usage across multiple LLM providers. Track your spending, monitor rate limits, and stay on top of your AI API consumption—all from a sleek, native interface.

## Architecture

ExactoBar follows a modular workspace architecture:

```
exactobar/
├── exactobar-core/       # Core types, models, and traits
├── exactobar-fetch/      # Fetch strategies and HTTP probes
├── exactobar-providers/  # Provider-specific implementations
├── exactobar-store/      # State management and persistence
├── exactobar-cli/        # Command-line interface
└── exactobar-app/        # GPUI desktop application
```

### Crate Responsibilities

| Crate | Purpose |
|-------|--------|
| `exactobar-core` | Shared types, domain models, and trait definitions |
| `exactobar-fetch` | HTTP client abstractions, retry strategies, rate limiting |
| `exactobar-providers` | Provider-specific API integrations and parsers |
| `exactobar-store` | In-memory and persistent state management |
| `exactobar-cli` | CLI for checking usage without the GUI |
| `exactobar-app` | Full GPUI menu bar application |

## Supported Providers

ExactoBar supports monitoring for the following LLM providers:

| Provider | Status | Description |
|----------|--------|-------------|
| **Codex** | 🟢 Planned | OpenAI Codex API |
| **Claude** | 🟢 Planned | Anthropic Claude API |
| **Cursor** | 🟢 Planned | Cursor IDE usage tracking |
| **Gemini** | 🟢 Planned | Google Gemini API |
| **Copilot** | 🟢 Planned | GitHub Copilot usage |
| **Factory** | 🟢 Planned | Factory AI platform |
| **VertexAI** | 🟢 Planned | Google Cloud Vertex AI |
| **z.ai** | 🟢 Planned | z.ai platform |
| **Augment** | 🟢 Planned | Augment Code |
| **Kiro** | 🟢 Planned | Kiro AI |
| **Antigravity** | 🟢 Planned | Antigravity AI |
| **MiniMax** | 🟢 Planned | MiniMax API |

## Building

### Prerequisites

- Rust 1.85.0 or later (stable)
- macOS 13.0+ (for GPUI)
- Xcode Command Line Tools

### Build Commands

```bash
# Build all crates
cargo build

# Build in release mode
cargo build --release

# Run the CLI
cargo run -p exactobar-cli -- --help

# Run the GUI app
cargo run -p exactobar-app

# Run tests
cargo test --workspace

# Check formatting
cargo fmt --all -- --check

# Run clippy lints
cargo clippy --workspace -- -D warnings
```

## CLI Usage Preview

```bash
# Check all configured providers
exactobar check

# Check a specific provider
exactobar check --provider claude

# Output as JSON
exactobar check --format json

# List configured providers
exactobar providers list

# Add a new provider
exactobar providers add claude --api-key "sk-..."

# Show usage summary
exactobar summary

# Watch mode (live updates)
exactobar watch --interval 30
```

## Configuration

ExactoBar stores its configuration in `~/.config/exactobar/config.toml`:

```toml
[general]
refresh_interval = 60  # seconds
show_notifications = true

[providers.claude]
enabled = true
api_key_env = "ANTHROPIC_API_KEY"

[providers.gemini]
enabled = true
api_key_env = "GOOGLE_API_KEY"
```

## Development

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p exactobar-core

# Run with logging
RUST_LOG=debug cargo test --workspace
```

### Project Principles

- **DRY**: Don't Repeat Yourself - shared code lives in `exactobar-core`
- **YAGNI**: You Aren't Gonna Need It - only implement what's needed
- **SOLID**: Single responsibility, Open/closed, Liskov substitution, Interface segregation, Dependency inversion
- **Zen of Python**: Even in Rust, we follow the Zen (explicit > implicit, simple > complex)

## Security

ExactoBar handles sensitive credentials securely:

### Credential Storage

- **Keychain Storage**: All OAuth tokens and API keys are stored in the system keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- **No Plain-Text Secrets**: Configuration files contain only non-sensitive settings (enabled providers, refresh intervals, UI preferences)
- **File Permissions**: Config files are created with `0600` permissions (owner read/write only) on Unix systems
- **Directory Permissions**: Config directories use `0700` permissions (owner access only)

### Network Security

- **TLS Only**: All network requests use TLS 1.3 via rustls (no OpenSSL dependency)
- **Certificate Verification**: System certificate store is used for validation
- **No HTTP Fallback**: Plain HTTP connections are never used for API calls

### Code Security

- **Memory Safety**: Zero `unsafe` code blocks in the entire codebase
- **Input Validation**: All API responses are validated before use
- **Atomic Writes**: Config files use atomic write operations (temp file + rename)

### Privacy Note

ExactoBar may read credentials stored by provider CLIs (e.g., Claude CLI's `~/.claude/config.json`, Cursor's local database). This is necessary for seamless integration but means:

1. Those files should also be protected with appropriate permissions
2. ExactoBar does not transmit these credentials anywhere except to the original provider APIs
3. Debug mode (`--debug-html`) can write sensitive HTML responses to disk - **do not use in production**

### Reporting Security Issues

If you discover a security vulnerability, please report it privately via GitHub Security Advisories rather than opening a public issue.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Original [CodexBar](https://github.com/example/codexbar) Swift implementation
- [GPUI](https://github.com/zed-industries/zed) framework by Zed Industries
- All the amazing LLM providers making AI accessible

---

*Built with 🦀 and ❤️*
