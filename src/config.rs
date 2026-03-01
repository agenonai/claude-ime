/// TOML configuration loader for claude-ime.
///
/// The configuration file lives at `~/.config/claude-ime/config.toml`.
/// All fields are optional; a missing file is treated the same as an empty
/// file (all defaults apply).  CLI flags always take precedence over values
/// read from disk.
use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{ClaudeImeError, Result};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Values that can be set in `~/.config/claude-ime/config.toml`.
///
/// ```toml
/// # Example configuration file
/// claude_path = "/opt/homebrew/bin/claude"
/// verbose = false
///
/// [extra_env]
/// CLAUDE_TELEMETRY = "off"
/// ```
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Explicit path to the `claude` binary.  Equivalent to `--claude-path`.
    pub claude_path: Option<String>,

    /// Enable debug-level logging by default.  Equivalent to `--verbose`.
    pub verbose: Option<bool>,

    /// Additional environment variables injected into the child process.
    /// Variables set here are merged with (and can be overridden by) the
    /// inherited environment.
    pub extra_env: Option<HashMap<String, String>>,
}

/// The final, resolved configuration after merging the config file with CLI
/// arguments.  All fields that require a value have been resolved to their
/// concrete types.
#[derive(Debug)]
pub struct ResolvedConfig {
    /// Path to the claude binary, if known at this stage.
    pub claude_path: Option<PathBuf>,
    /// Whether debug logging is enabled.
    pub verbose: bool,
    /// Extra environment variables to inject into the child process.
    pub extra_env: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Returns the path to the configuration file.
///
/// Uses the `dirs` crate to locate the user's configuration directory so that
/// the code is portable across Linux, macOS, and Windows.
fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join("claude-ime").join("config.toml"))
}

/// Load configuration from `~/.config/claude-ime/config.toml`.
///
/// Returns a default (all-`None`) [`Config`] when the file does not exist, so
/// callers never need to special-case a missing file.  Returns
/// [`ClaudeImeError::Config`] only when the file exists but cannot be parsed.
pub fn load() -> Result<Config> {
    let path = match config_path() {
        Some(p) => p,
        // Cannot determine home directory — silently use defaults.
        None => return Ok(Config::default()),
    };

    if !path.exists() {
        return Ok(Config::default());
    }

    let raw = std::fs::read_to_string(&path).map_err(|e| {
        ClaudeImeError::Config(format!("Cannot read {}: {}", path.display(), e))
    })?;

    toml::from_str::<Config>(&raw).map_err(|e| {
        ClaudeImeError::Config(format!(
            "Cannot parse {}: {}",
            path.display(),
            e
        ))
    })
}

// ---------------------------------------------------------------------------
// Merging
// ---------------------------------------------------------------------------

/// Merge a config file snapshot and CLI overrides into a [`ResolvedConfig`].
///
/// CLI values always win over config-file values, which in turn win over
/// compiled-in defaults.
///
/// # Arguments
///
/// * `file`         — Values loaded from the TOML file (use [`load`]).
/// * `cli_path`     — Value of `--claude-path`, if provided.
/// * `cli_verbose`  — Whether `-v` / `--verbose` was passed on the CLI.
pub fn merge(
    file: Config,
    cli_path: Option<PathBuf>,
    cli_verbose: bool,
) -> ResolvedConfig {
    // CLI --claude-path wins; fall back to the config-file string converted to
    // a PathBuf.
    let claude_path = cli_path.or_else(|| file.claude_path.map(PathBuf::from));

    // CLI --verbose wins; fall back to config-file value; default false.
    let verbose = cli_verbose || file.verbose.unwrap_or(false);

    let extra_env = file.extra_env.unwrap_or_default();

    ResolvedConfig {
        claude_path,
        verbose,
        extra_env,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(claude_path: Option<&str>, verbose: Option<bool>) -> Config {
        Config {
            claude_path: claude_path.map(String::from),
            verbose,
            extra_env: None,
        }
    }

    #[test]
    fn cli_verbose_overrides_config_false() {
        let cfg = make_config(None, Some(false));
        let resolved = merge(cfg, None, true);
        assert!(resolved.verbose);
    }

    #[test]
    fn config_verbose_used_when_cli_silent() {
        let cfg = make_config(None, Some(true));
        let resolved = merge(cfg, None, false);
        assert!(resolved.verbose);
    }

    #[test]
    fn defaults_when_both_silent() {
        let cfg = make_config(None, None);
        let resolved = merge(cfg, None, false);
        assert!(!resolved.verbose);
    }

    #[test]
    fn cli_path_overrides_config_path() {
        let cfg = make_config(Some("/config/claude"), None);
        let cli = Some(PathBuf::from("/cli/claude"));
        let resolved = merge(cfg, cli, false);
        assert_eq!(resolved.claude_path.unwrap(), PathBuf::from("/cli/claude"));
    }

    #[test]
    fn config_path_used_when_no_cli_path() {
        let cfg = make_config(Some("/config/claude"), None);
        let resolved = merge(cfg, None, false);
        assert_eq!(
            resolved.claude_path.unwrap(),
            PathBuf::from("/config/claude")
        );
    }

    #[test]
    fn extra_env_defaults_to_empty() {
        let cfg = Config::default();
        let resolved = merge(cfg, None, false);
        assert!(resolved.extra_env.is_empty());
    }

    #[test]
    fn extra_env_forwarded_from_config() {
        let mut env = HashMap::new();
        env.insert("FOO".to_string(), "bar".to_string());
        let cfg = Config {
            claude_path: None,
            verbose: None,
            extra_env: Some(env),
        };
        let resolved = merge(cfg, None, false);
        assert_eq!(resolved.extra_env.get("FOO").map(String::as_str), Some("bar"));
    }

    /// Verify that [`load`] does not panic or error on a missing file.  We
    /// cannot predict the test runner's home directory, so we just confirm the
    /// function returns `Ok`.
    #[test]
    fn load_missing_file_returns_default() {
        // load() reads from the real filesystem; if the file does not exist
        // it should succeed.  We cannot force a specific path here without
        // refactoring, but the function is written to return Ok(default) for a
        // missing file, so this test documents and guards that contract.
        //
        // If the tester happens to have the file, this still exercises the
        // parse path, which is also fine.
        let result = load();
        assert!(result.is_ok(), "load() must not fail on missing file: {result:?}");
    }
}
