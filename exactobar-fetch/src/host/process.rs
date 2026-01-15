//! Subprocess execution for CLI tools.
//!
//! This module provides utilities for running external commands,
//! particularly CLI tools like `claude`, `gh`, etc.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tracing::{debug, instrument, warn};

use crate::error::ProcessError;

/// Default command timeout.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

// ============================================================================
// Process Output
// ============================================================================

/// Output from a process execution.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    /// Standard output content.
    pub stdout: String,
    /// Standard error content.
    pub stderr: String,
    /// Exit code (0 = success).
    pub exit_code: i32,
    /// How long the command took to execute.
    pub duration: Duration,
}

impl ProcessOutput {
    /// Returns true if the command succeeded (exit code 0).
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    /// Returns the stdout if successful, otherwise an error.
    pub fn stdout_if_success(&self) -> Result<&str, ProcessError> {
        if self.success() {
            Ok(&self.stdout)
        } else {
            Err(ProcessError::NonZeroExit {
                code: self.exit_code,
                stderr: self.stderr.clone(),
            })
        }
    }
}

// ============================================================================
// Process Runner
// ============================================================================

/// API for running subprocesses (CLI tools).
#[derive(Debug, Clone, Default)]
pub struct ProcessRunner;

impl ProcessRunner {
    /// Creates a new process runner.
    pub fn new() -> Self {
        Self
    }

    /// Run a command and capture output.
    #[instrument(skip(self), fields(cmd = %cmd))]
    pub async fn run(&self, cmd: &str, args: &[&str]) -> Result<ProcessOutput, ProcessError> {
        self.run_with_timeout(cmd, args, Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .await
    }

    /// Run a command with timeout.
    #[instrument(skip(self), fields(cmd = %cmd, timeout = ?timeout))]
    pub async fn run_with_timeout(
        &self,
        cmd: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<ProcessOutput, ProcessError> {
        self.run_internal(cmd, args, &[], Some(timeout)).await
    }

    /// Run a command with environment variables.
    #[instrument(skip(self, env), fields(cmd = %cmd))]
    pub async fn run_with_env(
        &self,
        cmd: &str,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Result<ProcessOutput, ProcessError> {
        self.run_internal(cmd, args, env, Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS)))
            .await
    }

    /// Run a command with full options.
    #[instrument(skip(self, env), fields(cmd = %cmd))]
    pub async fn run_with_options(
        &self,
        cmd: &str,
        args: &[&str],
        env: &[(&str, &str)],
        timeout: Option<Duration>,
    ) -> Result<ProcessOutput, ProcessError> {
        self.run_internal(cmd, args, env, timeout).await
    }

    /// Internal implementation of process execution.
    async fn run_internal(
        &self,
        cmd: &str,
        args: &[&str],
        env: &[(&str, &str)],
        timeout: Option<Duration>,
    ) -> Result<ProcessOutput, ProcessError> {
        debug!(args = ?args, "Running command");

        // Find the command
        let cmd_path = self.which(cmd).ok_or_else(|| {
            warn!(cmd = %cmd, "Command not found");
            ProcessError::NotFound(cmd.to_string())
        })?;

        let start = Instant::now();

        // Build the command
        let mut command = Command::new(&cmd_path);
        command
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Add environment variables
        for (key, value) in env {
            command.env(key, value);
        }

        // Spawn and wait with optional timeout
        let output = if let Some(timeout) = timeout {
            match tokio::time::timeout(timeout, command.output()).await {
                Ok(result) => result?,
                Err(_) => {
                    warn!(cmd = %cmd, timeout = ?timeout, "Command timed out");
                    return Err(ProcessError::Timeout(timeout));
                }
            }
        } else {
            command.output().await?
        };

        let duration = start.elapsed();
        let exit_code = output.status.code().unwrap_or(-1);

        let result = ProcessOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code,
            duration,
        };

        debug!(
            exit_code = exit_code,
            duration = ?duration,
            stdout_len = result.stdout.len(),
            stderr_len = result.stderr.len(),
            "Command completed"
        );

        Ok(result)
    }

    /// Check if a command exists on PATH.
    pub fn command_exists(&self, cmd: &str) -> bool {
        self.which(cmd).is_some()
    }

    /// Find the path to a command.
    pub fn which(&self, cmd: &str) -> Option<PathBuf> {
        which::which(cmd).ok()
    }

    /// Find all instances of a command on PATH.
    pub fn which_all(&self, cmd: &str) -> Vec<PathBuf> {
        which::which_all(cmd)
            .map(|iter| iter.collect())
            .unwrap_or_default()
    }
}

// ============================================================================
// Common CLI Commands
// ============================================================================

/// Common CLI tool names.
pub mod commands {
    pub const CLAUDE: &str = "claude";
    pub const GH: &str = "gh";
    pub const GCLOUD: &str = "gcloud";
    pub const CURSOR: &str = "cursor";
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_exists() {
        let runner = ProcessRunner::new();

        // These should exist on most systems
        assert!(runner.command_exists("echo"));
        assert!(runner.command_exists("ls") || runner.command_exists("dir"));

        // This should not exist
        assert!(!runner.command_exists("definitely_not_a_real_command_12345"));
    }

    #[test]
    fn test_which() {
        let runner = ProcessRunner::new();

        // echo should be found
        let path = runner.which("echo");
        assert!(path.is_some());

        // Non-existent command
        let path = runner.which("not_a_command_xyz");
        assert!(path.is_none());
    }

    #[tokio::test]
    async fn test_run_echo() {
        let runner = ProcessRunner::new();

        let output = runner.run("echo", &["hello", "world"]).await.unwrap();

        assert!(output.success());
        assert!(output.stdout.trim() == "hello world");
        assert!(output.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_run_failure() {
        let runner = ProcessRunner::new();

        // ls on a non-existent path should fail
        let output = runner
            .run("ls", &["/definitely/not/a/real/path/12345"])
            .await
            .unwrap();

        assert!(!output.success());
        assert!(!output.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_run_not_found() {
        let runner = ProcessRunner::new();

        let result = runner.run("not_a_real_command_xyz", &[]).await;

        assert!(matches!(result, Err(ProcessError::NotFound(_))));
    }
}
