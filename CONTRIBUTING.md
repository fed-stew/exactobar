# Contributing to ExactoBar

Thank you for your interest in contributing to ExactoBar! 🎯

## Development Setup

### Prerequisites

- Rust 1.85.0 or later (stable)
- macOS 13.0+ (for GPUI framework)
- Xcode Command Line Tools

### Building

```bash
# Clone the repository
git clone https://github.com/janfeddersen/exactobar
cd exactobar

# Build all crates
cargo build

# Run tests
cargo test --workspace

# Run the CLI
cargo run -p exactobar-cli -- --help

# Run the GUI app
cargo run -p exactobar-app
```

## Code Style

We use the standard Rust tooling for code quality:

```bash
# Format code
cargo fmt --all

# Run clippy lints
cargo clippy --workspace -- -D warnings

# Run security audit
cargo deny check
```

### Lint Configuration

All crates use pedantic clippy lints. See `clippy.toml` and `rustfmt.toml` for project settings.

## Testing

- Unit tests are inline with `#[cfg(test)]` modules
- Integration tests go in `crate_name/tests/`
- Use `#[tokio::test]` for async tests
- Mock external services; don't make real network calls in tests

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Ensure all checks pass (`cargo fmt`, `cargo clippy`, `cargo test`)
5. Commit with clear messages
6. Push and open a Pull Request

## Adding a New Provider

1. Create a new module in `exactobar-providers/src/`
2. Implement the descriptor in `descriptor.rs`
3. Add fetch strategies (CLI, OAuth, Web, etc.)
4. Register in `registry.rs`
5. Add tests for parsing logic

## Security

- Never log credentials or tokens
- Use `#[instrument(skip(token))]` for tracing
- Store secrets in system keychain
- Set file permissions to 0o600 for sensitive files

Report security vulnerabilities via GitHub Security Advisories.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
