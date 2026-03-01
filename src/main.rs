//! claude-ime — PTY wrapper that fixes broken IME input for Claude Code.
//!
//! Type Vietnamese, Chinese, Japanese, Korean, and any other IME-composed
//! characters without garbling.  claude-ime intercepts stdin, detects
//! Unicode boundaries, and forwards complete code-point sequences to the
//! child process.

mod cli;
mod config;
mod error;
mod proxy;
mod pty;
mod signals;
mod terminal;
mod utf8;
mod version;

use clap::Parser;

use crate::cli::Cli;
use crate::error::{ClaudeImeError, Result};

fn main() {
    let cli = Cli::parse();

    let exit_code = match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("claude-ime: {e}");
            1
        }
    };

    std::process::exit(exit_code);
}

fn run(cli: Cli) -> Result<i32> {
    // Initialise logging — env_logger respects RUST_LOG; --verbose sets the
    // minimum level to DEBUG regardless of RUST_LOG.
    let log_level = if cli.verbose { "debug" } else { "warn" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::debug!("claude-ime {}", version::VERSION);

    // Load configuration and merge with CLI flags.
    let file_cfg = config::load()?;
    let resolved = config::merge(file_cfg, cli.claude_path, cli.verbose);

    // Determine the command to run.
    let command = resolve_command(&resolved, &cli.wrap)?;
    log::debug!("resolved command: {}", command.display());

    // Install signal handlers before touching the terminal.
    signals::setup_handlers()?;

    // Enter raw mode and keep the guard alive for the duration of the run.
    let saved = terminal::set_raw_mode()?;
    let _guard = terminal::CleanupGuard::new(saved);

    // Query the initial terminal size.
    let (rows, cols) = terminal::get_size()?;
    log::debug!("initial terminal size: {rows}x{cols}");

    // Build the command and spawn it inside a PTY.
    let program = command
        .to_str()
        .ok_or_else(|| ClaudeImeError::SpawnFailed("non-UTF-8 path".into()))?;

    let cmd = pty::build_command(program, &cli.args, &resolved.extra_env);
    let (pair, mut child) = pty::create(cmd, rows, cols)?;

    // Register the child PID for signal forwarding.
    if let Some(pid) = child.process_id() {
        signals::set_child_pid(pid as i32);
    }

    // Run the bidirectional proxy until the child exits.
    proxy::run(pair.master, &mut child)
}

/// Resolve the binary to run: `--wrap` > `--claude-path` > PATH discovery.
fn resolve_command(
    cfg: &config::ResolvedConfig,
    wrap: &Option<String>,
) -> Result<std::path::PathBuf> {
    if let Some(cmd) = wrap {
        // --wrap takes precedence; look it up on PATH.
        return which::which(cmd)
            .map_err(|_| ClaudeImeError::SpawnFailed(format!("command not found: {cmd}")));
    }

    if let Some(path) = &cfg.claude_path {
        return Ok(path.clone());
    }

    // Default: find `claude` on PATH.
    which::which("claude").map_err(|_| ClaudeImeError::ClaudeNotFound)
}
