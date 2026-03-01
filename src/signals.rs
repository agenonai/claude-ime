/// Signal handling for claude-ime.
///
/// Three signals require special handling in the PTY wrapper:
///
/// - `SIGWINCH` — terminal resize.  We set a flag that the proxy loop polls so
///   it can forward the new size to the PTY.
/// - `SIGINT`  — Ctrl-C.  We forward it to the child rather than letting the
///   default handler kill the wrapper process itself.
/// - `SIGTERM` — graceful termination.  Same forwarding logic as SIGINT.
///
/// All mutable state shared between signal handlers and the main thread is held
/// in `AtomicI32` / `AtomicBool` so the handler bodies are async-signal-safe.
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use nix::sys::signal::{self, SigHandler, Signal};
use nix::unistd::Pid;

use crate::error::Result;

// ────────────────────────────────────────────────────────────
//  Shared state
// ────────────────────────────────────────────────────────────

/// PID of the child process, or 0 if not yet set.
///
/// Written by the main thread once after `spawn`, read (and possibly acted on)
/// inside signal handlers.  `Relaxed` ordering is sufficient because signal
/// delivery already provides a happens-before edge on most platforms, and
/// correctness does not depend on ordering with other atomics.
static CHILD_PID: AtomicI32 = AtomicI32::new(0);

/// Set to `true` by the `SIGWINCH` handler; cleared by [`is_resize_pending`].
static RESIZE_PENDING: AtomicBool = AtomicBool::new(false);

// ────────────────────────────────────────────────────────────
//  Public API
// ────────────────────────────────────────────────────────────

/// Store the child PID so that signal handlers can forward signals to it.
///
/// Call this once, immediately after spawning the child process.
pub fn set_child_pid(pid: i32) {
    CHILD_PID.store(pid, Ordering::Relaxed);
}

/// Check whether a `SIGWINCH` has arrived since the last call.
///
/// Returns `true` and atomically resets the flag if a resize is pending,
/// otherwise returns `false`.  The main proxy loop calls this on each iteration
/// so it can update the PTY size without a dedicated thread.
pub fn is_resize_pending() -> bool {
    RESIZE_PENDING.swap(false, Ordering::AcqRel)
}

/// Install all signal handlers required by the PTY proxy.
///
/// Must be called from the main thread before spawning any threads that might
/// inherit signal masks.
///
/// # Errors
///
/// Returns an error if any `sigaction` / `signal` call fails.
pub fn setup_handlers() -> Result<()> {
    install_sigwinch()?;
    install_sigint()?;
    install_sigterm()?;
    Ok(())
}

// ────────────────────────────────────────────────────────────
//  SIGWINCH
// ────────────────────────────────────────────────────────────

/// Raw signal handler for `SIGWINCH`.
///
/// # Safety
///
/// Only async-signal-safe operations are used (`AtomicBool::store`).
extern "C" fn handle_sigwinch(_: nix::libc::c_int) {
    RESIZE_PENDING.store(true, Ordering::Release);
}

fn install_sigwinch() -> Result<()> {
    let handler = SigHandler::Handler(handle_sigwinch);
    // SAFETY: `handle_sigwinch` only calls async-signal-safe operations.
    unsafe { signal::signal(Signal::SIGWINCH, handler) }
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
    Ok(())
}

// ────────────────────────────────────────────────────────────
//  SIGINT
// ────────────────────────────────────────────────────────────

/// Raw signal handler for `SIGINT`.
///
/// Forwards the signal to the child process if one is registered.
///
/// # Safety
///
/// Uses only async-signal-safe operations (`AtomicI32::load`, `kill`).
extern "C" fn handle_sigint(_: nix::libc::c_int) {
    forward_signal_to_child(Signal::SIGINT);
}

fn install_sigint() -> Result<()> {
    let handler = SigHandler::Handler(handle_sigint);
    // SAFETY: `handle_sigint` only calls async-signal-safe operations.
    unsafe { signal::signal(Signal::SIGINT, handler) }
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
    Ok(())
}

// ────────────────────────────────────────────────────────────
//  SIGTERM
// ────────────────────────────────────────────────────────────

/// Raw signal handler for `SIGTERM`.
///
/// Forwards the signal to the child process if one is registered.
///
/// # Safety
///
/// Uses only async-signal-safe operations.
extern "C" fn handle_sigterm(_: nix::libc::c_int) {
    forward_signal_to_child(Signal::SIGTERM);
}

fn install_sigterm() -> Result<()> {
    let handler = SigHandler::Handler(handle_sigterm);
    // SAFETY: `handle_sigterm` only calls async-signal-safe operations.
    unsafe { signal::signal(Signal::SIGTERM, handler) }
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
    Ok(())
}

// ────────────────────────────────────────────────────────────
//  Internal helpers
// ────────────────────────────────────────────────────────────

/// Forward `sig` to the registered child PID, if any.
///
/// This function must only be called from signal handler context; it only uses
/// async-signal-safe operations.
fn forward_signal_to_child(sig: Signal) {
    let raw_pid = CHILD_PID.load(Ordering::Relaxed);
    if raw_pid > 0 {
        let pid = Pid::from_raw(raw_pid);
        // Errors (e.g. ESRCH if the child already exited) are silently ignored
        // because there is no async-signal-safe way to report them.
        let _ = signal::kill(pid, sig);
    }
}

