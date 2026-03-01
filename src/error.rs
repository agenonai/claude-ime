/// Error types for claude-ime.
///
/// All public-facing errors carry enough context to produce actionable
/// messages without requiring callers to understand internal state.
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClaudeImeError {
    /// A PTY could not be created or the child process could not be spawned
    /// into it.  The inner string contains the underlying cause.
    #[error("PTY error: {0}")]
    Pty(String),

    /// Any standard I/O error that bubbles up from the OS or from reading /
    /// writing the PTY file descriptors.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The configuration file exists but could not be parsed, or a required
    /// configuration value is invalid.
    #[error("Configuration error: {0}")]
    Config(String),

    /// The `claude` binary was not found on PATH and no explicit path was
    /// provided via `--claude-path`.
    #[error(
        "Claude Code not found. Install it from https://docs.anthropic.com/en/docs/claude-code"
    )]
    ClaudeNotFound,

    /// The child process could not be started.  The inner string describes
    /// what was attempted and why it failed.
    #[error("Failed to spawn child process: {0}")]
    SpawnFailed(String),
}

/// Convenience alias so callers can write `Result<T>` instead of
/// `Result<T, ClaudeImeError>`.
pub type Result<T> = std::result::Result<T, ClaudeImeError>;
