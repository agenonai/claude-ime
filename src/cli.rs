/// Command-line interface definition for claude-ime.
///
/// `claude-ime` is a PTY wrapper that transparently forwards IME-composed
/// characters (Vietnamese, Chinese, Japanese, Korean, etc.) to Claude Code
/// without the garbling that occurs when the host terminal and the inner pty
/// disagree on encoding boundaries.
use std::path::PathBuf;

use clap::Parser;

use crate::version::VERSION;

/// Fix broken IME / multi-byte input for Claude Code (and any other terminal
/// program).
///
/// By default claude-ime launches `claude` and proxies all I/O through a
/// managed PTY, normalising Unicode boundaries so that Vietnamese, CJK, and
/// other IME-composed characters are delivered to the process intact.
///
/// # Examples
///
/// ```text
/// # Run Claude Code with IME fix (default)
/// claude-ime
///
/// # Pass extra arguments straight through to claude
/// claude-ime -- --resume
///
/// # Wrap an arbitrary command instead of claude
/// claude-ime --wrap bash
///
/// # Use a non-standard claude binary
/// claude-ime --claude-path /opt/homebrew/bin/claude
/// ```
#[derive(Debug, Parser)]
#[command(
    name = "claude-ime",
    version = VERSION,
    about = "Fix IME / multi-byte input for Claude Code (Vietnamese, CJK, and more)",
    long_about = concat!(
        "claude-ime wraps Claude Code (or any command) in a managed PTY and \n",
        "normalises Unicode / IME input boundaries so that multi-byte characters \n",
        "typed via an Input Method Editor — Vietnamese Telex/VNI, Chinese Pinyin, \n",
        "Japanese Hiragana, Korean Hangul, etc. — are forwarded to the child \n",
        "process intact rather than being split across multiple reads.\n\n",
        "If you just want to run Claude Code with the fix applied, run:\n\n",
        "    claude-ime\n\n",
        "To wrap a different program use --wrap:\n\n",
        "    claude-ime --wrap bash\n\n",
        "Any extra arguments after -- are forwarded verbatim to the wrapped command.",
    ),
    after_help = "Project home: https://github.com/agenon/claude-ime",
)]
pub struct Cli {
    /// Enable debug-level logging.
    ///
    /// When set, claude-ime prints PTY events, byte counts, and UTF-8
    /// boundary decisions to stderr.  This is useful when diagnosing
    /// garbled-character issues.
    #[arg(short, long, help = "Enable debug logging (PTY events, byte counts)")]
    pub verbose: bool,

    /// Explicit path to the `claude` binary.
    ///
    /// Overrides automatic PATH discovery.  Use this when you have multiple
    /// Claude Code installations or the binary is not on PATH.
    #[arg(
        long,
        value_name = "PATH",
        help = "Path to the claude binary (overrides PATH discovery)"
    )]
    pub claude_path: Option<PathBuf>,

    /// Wrap an arbitrary command instead of `claude`.
    ///
    /// The value is the program name or path; use `--` to pass arguments
    /// through.
    ///
    /// Example: `claude-ime --wrap bash -- -i`
    #[arg(
        long,
        value_name = "COMMAND",
        help = "Wrap this command instead of claude (e.g. --wrap bash)"
    )]
    pub wrap: Option<String>,

    /// Arguments forwarded verbatim to the wrapped command.
    ///
    /// Separate claude-ime flags from forwarded arguments with `--`:
    ///
    ///     claude-ime --verbose -- --resume --model sonnet
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS",
        help = "Arguments passed through to the wrapped command"
    )]
    pub args: Vec<String>,
}
