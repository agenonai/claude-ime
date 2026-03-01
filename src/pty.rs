/// PTY creation and management.
///
/// This module is a thin, ergonomic wrapper around [`portable_pty`] that
/// provides the three operations the rest of claude-ime needs:
///
/// 1. [`build_command`] — create a `CommandBuilder` from a program name, its
///    arguments, and an optional set of extra environment variables.
/// 2. [`create`] — open a native PTY pair and spawn the command on the slave
///    side.
/// 3. [`resize`] — update the PTY's advertised window size after a `SIGWINCH`.
use std::collections::HashMap;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, PtyPair};

use crate::error::{ClaudeImeError, Result};

// ────────────────────────────────────────────────────────────
//  Command builder
// ────────────────────────────────────────────────────────────

/// Build a [`CommandBuilder`] from a program path, argument list, and a map of
/// extra environment variables to inject.
///
/// The child process inherits the parent's full environment; `extra_env` only
/// *adds* or *overrides* individual variables.
///
/// # Parameters
///
/// - `program` — the executable to run (path or bare name looked up via PATH).
/// - `args`    — zero or more arguments forwarded verbatim to the command.
/// - `extra_env` — additional environment variables merged on top of the
///   inherited environment.
pub fn build_command(
    program: &str,
    args: &[String],
    extra_env: &HashMap<String, String>,
) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(program);

    for arg in args {
        cmd.arg(arg);
    }

    for (key, value) in extra_env {
        cmd.env(key, value);
    }

    cmd
}

// ────────────────────────────────────────────────────────────
//  PTY creation
// ────────────────────────────────────────────────────────────

/// Open a native PTY pair and spawn `cmd` on the slave side.
///
/// Returns the PTY pair (master + slave) and a handle to the spawned child.
/// The caller owns both and is responsible for:
///
/// - Reading from / writing to `pair.master`.
/// - Waiting for `child` to exit.
/// - Dropping `pair.slave` as soon as the child is spawned (the slave end
///   should not be held open by the parent process, or `read` on the master
///   will never see EOF).
///
/// # Errors
///
/// Returns [`ClaudeImeError::Pty`] if the PTY system fails to open a pair or
/// if spawning the child fails.
pub fn create(
    cmd: CommandBuilder,
    rows: u16,
    cols: u16,
) -> Result<(PtyPair, Box<dyn Child + Send + Sync>)> {
    let pty_system = portable_pty::native_pty_system();

    let size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pair = pty_system
        .openpty(size)
        .map_err(|e| ClaudeImeError::Pty(e.to_string()))?;

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| ClaudeImeError::SpawnFailed(e.to_string()))?;

    Ok((pair, child))
}

// ────────────────────────────────────────────────────────────
//  PTY resize
// ────────────────────────────────────────────────────────────

/// Notify the PTY master of a new terminal window size.
///
/// Call this whenever a `SIGWINCH` has been received and the real terminal size
/// has been re-queried via [`crate::terminal::get_size`].
///
/// # Errors
///
/// Returns [`ClaudeImeError::Pty`] if the underlying resize operation fails.
pub fn resize(master: &dyn MasterPty, rows: u16, cols: u16) -> Result<()> {
    let size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    master
        .resize(size)
        .map_err(|e| ClaudeImeError::Pty(e.to_string()))?;

    Ok(())
}
