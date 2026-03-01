/// Bidirectional I/O proxy between the host terminal and the PTY child.
///
/// Two OS threads move bytes in opposite directions:
///
/// - **stdin → PTY** (thread 1): reads from [`std::io::Stdin`] and writes
///   to the PTY master writer.  This direction is straightforward — we pass
///   bytes through as-is so that every keystroke reaches the child process.
///
/// - **PTY → stdout** (thread 2): reads from the PTY master reader, locates
///   safe UTF-8 boundaries using [`crate::utf8::find_safe_boundary`], and
///   writes only complete code-point sequences to stdout.  Any trailing bytes
///   that form an incomplete sequence are held in a small "remainder" buffer
///   and prepended to the next read.  This is the IME fix: it prevents
///   garbled CJK / Vietnamese characters caused by multi-byte sequences that
///   straddle PTY read boundaries.
///
/// The main thread (the caller of [`run`]) polls for two events while both
/// threads are alive:
///
/// 1. A pending `SIGWINCH` — when detected, the current terminal size is
///    queried and forwarded to the PTY so that the child's view of the window
///    matches the host terminal.
/// 2. Child exit — [`portable_pty::Child::try_wait`] is used to detect this
///    without blocking.  Once the child exits its code is returned.
use std::io::{Read, Write};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use portable_pty::{Child, MasterPty};

use crate::error::Result;
use crate::{signals, terminal, utf8};

/// Buffer size for both the stdin→PTY and PTY→stdout copy loops.
const BUF_SIZE: usize = 4096;

/// How long the main thread sleeps between resize / exit-status polls.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Maximum size of the UTF-8 remainder buffer (one code point is at most 4
/// bytes, so 8 bytes is more than enough).
const REMAINDER_CAP: usize = 8;

// ────────────────────────────────────────────────────────────
//  Public entry point
// ────────────────────────────────────────────────────────────

/// Run the bidirectional PTY proxy and return the child's exit code.
///
/// Blocks until the child process exits.  Signal handlers installed by
/// [`crate::signals::setup_handlers`] must be active before calling this
/// function.
///
/// # Parameters
///
/// - `master` — the PTY master obtained from [`crate::pty::create`].
/// - `child`  — the spawned child process.
///
/// # Errors
///
/// Returns an error if the PTY reader or writer cannot be obtained, or if
/// waiting for the child fails.
pub fn run(
    master: Box<dyn MasterPty + Send>,
    child: &mut Box<dyn Child + Send + Sync>,
) -> Result<i32> {
    // Obtain reader and writer from the master before moving it into Arc.
    // `try_clone_reader` gives us an independent `Read` handle; `take_writer`
    // consumes the write half out of the master.
    let pty_reader = master
        .try_clone_reader()
        .map_err(|e| crate::error::ClaudeImeError::Pty(e.to_string()))?;

    let pty_writer = master
        .take_writer()
        .map_err(|e| crate::error::ClaudeImeError::Pty(e.to_string()))?;

    // Wrap the master in an Arc so we can call `resize` from the main thread
    // while the reader/writer threads are alive.
    let master_arc: Arc<dyn MasterPty + Send + Sync> = {
        // portable-pty's `MasterPty` is not `Sync`, but we only ever call
        // `resize` from one thread (the main thread) while the reader and
        // writer use their own cloned handles.  We impose the `Sync` bound
        // here via a wrapper so Rust's type system is satisfied.
        //
        // SAFETY: We uphold the invariant by only calling `resize` from the
        // main thread.
        Arc::new(SyncMaster::new(master))
    };

    // ── Thread 1: stdin → PTY writer ────────────────────────────────────────
    let stdin_thread = {
        let mut writer = pty_writer;
        thread::Builder::new()
            .name("stdin-to-pty".into())
            .spawn(move || {
                let stdin = std::io::stdin();
                let mut stdin = stdin.lock();
                let mut buf = [0u8; BUF_SIZE];
                loop {
                    let n = match stdin.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    if writer.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                log::debug!("stdin→PTY thread exiting");
            })
            .map_err(crate::error::ClaudeImeError::Io)?
    };

    // ── Thread 2: PTY reader → stdout ────────────────────────────────────────
    let stdout_thread = {
        let mut reader = pty_reader;
        thread::Builder::new()
            .name("pty-to-stdout".into())
            .spawn(move || {
                let stdout = std::io::stdout();
                let mut stdout = stdout.lock();

                // A small buffer that holds the tail of the previous read when
                // it ended in the middle of a multi-byte UTF-8 sequence.
                let mut remainder: Vec<u8> = Vec::with_capacity(REMAINDER_CAP);

                let mut raw_buf = [0u8; BUF_SIZE];

                loop {
                    let n = match reader.read(&mut raw_buf) {
                        Ok(0) => break, // PTY EOF — child closed its end
                        Ok(n) => n,
                        Err(e) => {
                            // EIO is normal when the child exits and the slave
                            // side of the PTY is closed.
                            log::debug!("PTY read error (expected on child exit): {e}");
                            break;
                        }
                    };

                    log::debug!("PTY read {} bytes", n);

                    // Combine any leftover bytes from the previous iteration
                    // with the freshly-read bytes.
                    let total_len = remainder.len() + n;
                    let mut combined = Vec::with_capacity(total_len);
                    combined.extend_from_slice(&remainder);
                    combined.extend_from_slice(&raw_buf[..n]);

                    // Find the largest prefix that ends on a complete UTF-8
                    // boundary.
                    let safe = utf8::find_safe_boundary(&combined, combined.len());

                    log::debug!("UTF-8 boundary: safe={safe} / total={}", combined.len());

                    if safe > 0 {
                        if stdout.write_all(&combined[..safe]).is_err() {
                            break;
                        }
                        if stdout.flush().is_err() {
                            break;
                        }
                    }

                    // Keep the bytes after the safe boundary for next time.
                    remainder = combined[safe..].to_vec();

                    // Guard against a runaway remainder (should never exceed 3
                    // bytes for any valid UTF-8 stream).
                    if remainder.len() > REMAINDER_CAP {
                        log::debug!("Flushing oversized remainder ({} bytes)", remainder.len());
                        let _ = stdout.write_all(&remainder);
                        let _ = stdout.flush();
                        remainder.clear();
                    }
                }

                // Flush any remaining bytes before the thread exits.
                if !remainder.is_empty() {
                    let _ = stdout.write_all(&remainder);
                    let _ = stdout.flush();
                }

                log::debug!("PTY→stdout thread exiting");
            })
            .map_err(crate::error::ClaudeImeError::Io)?
    };

    // ── Main thread: resize polling + child wait ─────────────────────────────
    let exit_code = loop {
        thread::sleep(POLL_INTERVAL);

        // Forward pending SIGWINCH to the PTY.
        if signals::is_resize_pending() {
            match terminal::get_size() {
                Ok((rows, cols)) => {
                    log::debug!("SIGWINCH: resizing PTY to {rows}×{cols}");
                    if let Err(e) = master_arc.resize(portable_pty::PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    }) {
                        log::debug!("PTY resize failed: {e}");
                    }
                }
                Err(e) => log::debug!("get_size failed: {e}"),
            }
        }

        // Check whether the child has exited.
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = exit_status_to_code(&status);
                log::info!("Child exited with status {code}");
                break code;
            }
            Ok(None) => {
                // Child still running — keep polling.
            }
            Err(e) => {
                log::debug!("try_wait error: {e}");
                break 1;
            }
        }
    };

    // The stdin thread blocks on stdin.read() indefinitely after the child
    // exits (there is no portable way to interrupt it).  We detach it and let
    // the process exit tear it down naturally.
    drop(stdin_thread);

    // Wait for the PTY→stdout thread to drain any buffered output.
    let _ = stdout_thread.join();

    Ok(exit_code)
}

// ────────────────────────────────────────────────────────────
//  Helpers
// ────────────────────────────────────────────────────────────

/// Extract a numeric exit code from a [`portable_pty::ExitStatus`].
fn exit_status_to_code(status: &portable_pty::ExitStatus) -> i32 {
    // portable_pty's ExitStatus implements Display as "ExitStatus(N)" and
    // does not expose a numeric accessor in 0.8.x.  We parse the Display
    // representation as a fallback.  The `success()` method is the reliable
    // part of the public API.
    if status.success() {
        0
    } else {
        // Attempt to extract the numeric code from the Display string.
        let s = status.to_string();
        s.trim_start_matches("ExitStatus(")
            .trim_end_matches(')')
            .parse::<i32>()
            .unwrap_or(1)
    }
}

// ────────────────────────────────────────────────────────────
//  SyncMaster wrapper
// ────────────────────────────────────────────────────────────

/// Newtype that implements `Sync` for `Box<dyn MasterPty + Send>`.
///
/// Uses `UnsafeCell` for interior mutability so `take_writer` can be
/// called through a shared reference (it is only called once, before
/// the `Arc<SyncMaster>` is shared across threads).
struct SyncMaster {
    inner: std::cell::UnsafeCell<Box<dyn MasterPty + Send>>,
}

// SAFETY: `resize` is only called from the main thread while reader/writer
// threads use their independently-cloned handles. `take_writer` is called
// exactly once before the Arc is shared. No concurrent mutation occurs.
unsafe impl Sync for SyncMaster {}

impl SyncMaster {
    fn new(master: Box<dyn MasterPty + Send>) -> Self {
        Self {
            inner: std::cell::UnsafeCell::new(master),
        }
    }

    /// Get a shared reference to the inner master.
    ///
    /// # Safety
    /// Caller must ensure no mutable access is happening concurrently.
    unsafe fn inner(&self) -> &dyn MasterPty {
        &**self.inner.get()
    }

    /// Get a mutable reference to the inner master.
    ///
    /// # Safety
    /// Caller must ensure no other access (shared or mutable) is happening
    /// concurrently. Only used for `take_writer` before the Arc is shared.
    #[allow(clippy::mut_from_ref)]
    unsafe fn inner_mut(&self) -> &mut Box<dyn MasterPty + Send> {
        &mut *self.inner.get()
    }
}

impl MasterPty for SyncMaster {
    fn resize(&self, size: portable_pty::PtySize) -> anyhow::Result<()> {
        // SAFETY: resize is only called from the main thread.
        unsafe { self.inner() }.resize(size)
    }

    fn get_size(&self) -> anyhow::Result<portable_pty::PtySize> {
        unsafe { self.inner() }.get_size()
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn Read + Send>> {
        unsafe { self.inner() }.try_clone_reader()
    }

    fn take_writer(&self) -> anyhow::Result<Box<dyn Write + Send>> {
        // SAFETY: take_writer is called exactly once, before the Arc is
        // shared across threads, so no aliased access occurs.
        unsafe { self.inner_mut() }.take_writer()
    }

    fn process_group_leader(&self) -> Option<i32> {
        unsafe { self.inner() }.process_group_leader()
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
        unsafe { self.inner() }.as_raw_fd()
    }
}
