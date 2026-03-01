/// Terminal state management for claude-ime.
///
/// This module handles saving and restoring terminal attributes, switching to
/// raw mode so the PTY wrapper can forward every byte unmodified, and querying
/// the current window size so the child PTY can be sized to match.
use std::os::unix::io::{BorrowedFd, RawFd};

use nix::sys::termios::{self, SetArg, Termios};

use crate::error::Result;

/// Borrow stdin as an `AsFd`-compatible reference for nix 0.29+ termios calls.
fn stdin_fd() -> BorrowedFd<'static> {
    // SAFETY: fd 0 (stdin) is always open for the lifetime of the process.
    unsafe { BorrowedFd::borrow_raw(0) }
}

/// Raw file descriptor for stdin, used for ioctl calls that need `RawFd`.
const STDIN_RAW_FD: RawFd = 0;

// ────────────────────────────────────────────────────────────
//  winsize ioctl
// ────────────────────────────────────────────────────────────

// We read the terminal window size via the TIOCGWINSZ ioctl.  `nix` exposes
// the `ioctl_read_bad!` macro for ioctls whose request code is obtained from a
// C header (not computed by the kernel's _IOC macro).
nix::ioctl_read_bad!(tiocgwinsz, nix::libc::TIOCGWINSZ, nix::libc::winsize);

// ────────────────────────────────────────────────────────────
//  TerminalState
// ────────────────────────────────────────────────────────────

/// A snapshot of the terminal's `termios` attributes.
///
/// Obtained via [`save`] and restored via [`TerminalState::restore`] or
/// automatically on drop through a [`CleanupGuard`].
#[derive(Clone)]
pub struct TerminalState {
    termios: Termios,
}

/// Save the current terminal attributes of stdin.
///
/// # Errors
///
/// Returns [`ClaudeImeError::Io`] if `tcgetattr` fails (e.g. stdin is not a
/// TTY).
pub fn save() -> Result<TerminalState> {
    let termios = termios::tcgetattr(stdin_fd())
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
    Ok(TerminalState { termios })
}

impl TerminalState {
    /// Restore the terminal to the attributes captured at construction time.
    ///
    /// Errors are intentionally swallowed here because `restore` is most often
    /// called from a `Drop` implementation where there is no useful way to
    /// propagate them.  If you need the error, use the free function variant.
    pub fn restore(&self) {
        let _ = termios::tcsetattr(stdin_fd(), SetArg::TCSAFLUSH, &self.termios);
    }
}

// ────────────────────────────────────────────────────────────
//  Raw mode
// ────────────────────────────────────────────────────────────

/// Switch stdin to raw mode and return the previous terminal state.
///
/// Raw mode means:
/// - No line buffering — every keystroke is available immediately.
/// - No echo — the PTY child controls what appears on screen.
/// - No signal generation from Ctrl-C / Ctrl-Z — we handle those ourselves.
///
/// The returned [`TerminalState`] must be restored before the process exits.
/// Prefer wrapping it in a [`CleanupGuard`] to ensure restoration even if the
/// process panics.
///
/// # Errors
///
/// Returns an error if either `tcgetattr` or `tcsetattr` fails.
pub fn set_raw_mode() -> Result<TerminalState> {
    let saved = save()?;

    let mut raw = saved.termios.clone();
    termios::cfmakeraw(&mut raw);

    termios::tcsetattr(stdin_fd(), SetArg::TCSAFLUSH, &raw)
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

    Ok(saved)
}

// ────────────────────────────────────────────────────────────
//  Window size
// ────────────────────────────────────────────────────────────

/// Query the current terminal window size.
///
/// Returns `(rows, cols)`.
///
/// # Errors
///
/// Returns an error if the `TIOCGWINSZ` ioctl fails (e.g. stdin is not a TTY,
/// or the platform does not support the ioctl).
pub fn get_size() -> Result<(u16, u16)> {
    let mut ws = nix::libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    // SAFETY: `ws` is a valid, stack-allocated `winsize` struct.  The ioctl
    // writes only `sizeof(winsize)` bytes through the pointer.
    unsafe { tiocgwinsz(STDIN_RAW_FD, &mut ws) }
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

    // Fall back to 80×24 if the kernel reports zeroes (e.g. in a pipe).
    let rows = if ws.ws_row == 0 { 24 } else { ws.ws_row };
    let cols = if ws.ws_col == 0 { 80 } else { ws.ws_col };

    Ok((rows, cols))
}

// ────────────────────────────────────────────────────────────
//  CleanupGuard — RAII terminal restoration
// ────────────────────────────────────────────────────────────

/// RAII guard that restores terminal attributes on drop.
///
/// Construct this immediately after entering raw mode so that the terminal is
/// always restored — even if the process panics or returns early.
///
/// ```no_run
/// use claude_ime::terminal::{CleanupGuard, set_raw_mode};
///
/// let saved = set_raw_mode()?;
/// let _guard = CleanupGuard::new(saved);
/// // ... do PTY work ...
/// // Terminal is restored here, even on panic.
/// ```
pub struct CleanupGuard {
    state: TerminalState,
}

impl CleanupGuard {
    /// Wrap a [`TerminalState`] in a guard that restores it on drop.
    pub fn new(state: TerminalState) -> Self {
        Self { state }
    }

    /// Consume the guard and restore the terminal immediately.
    ///
    /// Equivalent to dropping the guard, but makes the intent explicit.
    pub fn restore_now(self) {
        self.state.restore();
        // Prevent the Drop impl from restoring a second time.
        std::mem::forget(self);
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        self.state.restore();
    }
}

// ────────────────────────────────────────────────────────────
//  Helpers used internally by other modules
// ────────────────────────────────────────────────────────────

/// Apply a new window size to the PTY master file descriptor.
///
/// This is a thin wrapper around the `TIOCSWINSZ` ioctl and is used by the
/// signal handler after a `SIGWINCH` is received.
pub fn apply_size_to_fd(fd: RawFd, rows: u16, cols: u16) -> Result<()> {
    nix::ioctl_write_ptr_bad!(tiocswinsz, nix::libc::TIOCSWINSZ, nix::libc::winsize);

    let ws = nix::libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    // SAFETY: `ws` is valid and the ioctl only reads from it.
    unsafe { tiocswinsz(fd, &ws) }
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

    Ok(())
}

