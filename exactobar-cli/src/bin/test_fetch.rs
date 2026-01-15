//! Live test binary for fetching from installed providers.
//!
//! Run with: cargo run -p exactobar-cli --bin test_fetch

use exactobar_providers::claude::ClaudeUsageFetcher;
use exactobar_providers::codex::CodexUsageFetcher;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize logging (set RUST_LOG=debug for more output)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    println!("╔══════════════════════════════════════════════════════╗");
    println!("║     ExactoBar LIVE FETCH TEST                        ║");
    println!("╚══════════════════════════════════════════════════════╝");
    println!();
    println!("Testing REAL fetch from installed providers...\n");

    // Check what's installed
    let claude_available = which::which("claude").is_ok();
    let codex_available = which::which("codex").is_ok();

    println!("Detected CLIs:");
    println!(
        "  claude: {}",
        if claude_available {
            "✓ installed"
        } else {
            "✗ not found"
        }
    );
    println!(
        "  codex:  {}",
        if codex_available {
            "✓ installed"
        } else {
            "✗ not found"
        }
    );
    println!();

    // ========================================================================
    // Claude
    // ========================================================================
    println!("══════════════════════════════════════════════════════════");
    println!("  CLAUDE");
    println!("══════════════════════════════════════════════════════════");

    if !claude_available {
        println!("⏭️  Skipping - claude CLI not installed");
    } else {
        // Try to detect version
        if let Some(version) = ClaudeUsageFetcher::detect_version() {
            println!("Version: {}", version);
        }

        println!("Fetching usage (trying OAuth → Web → PTY)...");
        println!();

        let start = std::time::Instant::now();
        match ClaudeUsageFetcher::new().fetch_usage().await {
            Ok(snapshot) => {
                let elapsed = start.elapsed();
                println!("✅ SUCCESS! (took {:.2}s)", elapsed.as_secs_f64());
                println!();

                println!("Source: {:?}", snapshot.fetch_source);

                if let Some(primary) = &snapshot.primary {
                    println!();
                    println!("Session Usage:");
                    println!("  Used: {:.1}%", primary.used_percent);
                    println!("  Remaining: {:.1}%", 100.0 - primary.used_percent);
                    if let Some(desc) = &primary.reset_description {
                        println!("  Resets: {}", desc);
                    }
                    if let Some(mins) = primary.window_minutes {
                        println!("  Window: {} minutes ({:.1} hours)", mins, mins as f64 / 60.0);
                    }
                }

                if let Some(secondary) = &snapshot.secondary {
                    println!();
                    println!("Weekly Usage:");
                    println!("  Used: {:.1}%", secondary.used_percent);
                    println!("  Remaining: {:.1}%", 100.0 - secondary.used_percent);
                    if let Some(desc) = &secondary.reset_description {
                        println!("  Resets: {}", desc);
                    }
                }

                if let Some(tertiary) = &snapshot.tertiary {
                    println!();
                    println!("Opus/Sonnet Usage:");
                    println!("  Used: {:.1}%", tertiary.used_percent);
                    println!("  Remaining: {:.1}%", 100.0 - tertiary.used_percent);
                }

                if let Some(identity) = &snapshot.identity {
                    println!();
                    println!("Account:");
                    if let Some(email) = &identity.account_email {
                        println!("  Email: {}", email);
                    }
                    if let Some(org) = &identity.account_organization {
                        println!("  Organization: {}", org);
                    }
                    if let Some(plan) = &identity.plan_name {
                        println!("  Plan: {}", plan);
                    }
                }
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!("❌ Error (after {:.2}s): {}", elapsed.as_secs_f64(), e);
                println!();
                println!("This could mean:");
                println!("  • Not logged in to Claude CLI");
                println!("  • OAuth token expired");
                println!("  • No browser cookies available");
                println!("  • PTY probe failed");
            }
        }
    }

    println!();

    // ========================================================================
    // Codex
    // ========================================================================
    println!("══════════════════════════════════════════════════════════");
    println!("  CODEX");
    println!("══════════════════════════════════════════════════════════");

    if !codex_available {
        println!("⏭️  Skipping - codex CLI not installed");
    } else {
        // Try to detect version
        if let Some(version) = CodexUsageFetcher::detect_version() {
            println!("Version: {}", version);
        }

        println!("Fetching usage (trying RPC → PTY → API)...");
        println!();

        let start = std::time::Instant::now();
        match CodexUsageFetcher::new().fetch_usage().await {
            Ok(snapshot) => {
                let elapsed = start.elapsed();
                println!("✅ SUCCESS! (took {:.2}s)", elapsed.as_secs_f64());
                println!();

                println!("Source: {:?}", snapshot.fetch_source);

                if let Some(primary) = &snapshot.primary {
                    println!();
                    println!("5-Hour Usage:");
                    println!("  Used: {:.1}%", primary.used_percent);
                    println!("  Remaining: {:.1}%", 100.0 - primary.used_percent);
                    if let Some(desc) = &primary.reset_description {
                        println!("  Resets: {}", desc);
                    }
                }

                if let Some(secondary) = &snapshot.secondary {
                    println!();
                    println!("Weekly Usage:");
                    println!("  Used: {:.1}%", secondary.used_percent);
                    println!("  Remaining: {:.1}%", 100.0 - secondary.used_percent);
                    if let Some(desc) = &secondary.reset_description {
                        println!("  Resets: {}", desc);
                    }
                }

                // Credits would be in tertiary window for Codex
                if let Some(tertiary) = &snapshot.tertiary {
                    println!();
                    println!("Credits/Additional:");
                    println!("  Used: {:.1}%", tertiary.used_percent);
                    println!("  Remaining: {:.1}%", 100.0 - tertiary.used_percent);
                }

                if let Some(identity) = &snapshot.identity {
                    println!();
                    println!("Account:");
                    if let Some(email) = &identity.account_email {
                        println!("  Email: {}", email);
                    }
                    if let Some(org) = &identity.account_organization {
                        println!("  Organization: {}", org);
                    }
                }
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!("❌ Error (after {:.2}s): {}", elapsed.as_secs_f64(), e);
                println!();
                println!("This could mean:");
                println!("  • Not logged in to Codex CLI");
                println!("  • app-server mode not supported");
                println!("  • PTY probe failed");
            }
        }
    }

    println!();
    println!("══════════════════════════════════════════════════════════");
    println!("  TEST COMPLETE");
    println!("══════════════════════════════════════════════════════════");
}
