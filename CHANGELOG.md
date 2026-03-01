# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-01

### Added
- Initial release
- PTY wrapper for Claude Code with UTF-8 boundary handling
- Support for Vietnamese, Chinese, Japanese, and Korean IME input
- Configuration via `~/.config/claude-ime/config.toml`
- `--wrap` flag to wrap any command (not just Claude Code)
- `--verbose` flag for debugging IME composition events
- npm distribution via `@agenon/claude-ime` with auto-downloading precompiled binaries
- GitHub Actions CI/CD with 5-target release builds (darwin-x64, darwin-arm64, linux-x64, linux-arm64, win32-x64)
- Comprehensive test suite (unit + integration)
- Documentation: README, CONTRIBUTING, CHANGELOG
