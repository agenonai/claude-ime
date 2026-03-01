# claude-ime

**Fix IME input for Claude Code** — type Vietnamese, Chinese, Japanese, Korean, and more without garbled characters.

[![CI](https://github.com/agenon/claude-ime/workflows/CI/badge.svg)](https://github.com/agenon/claude-ime/actions)
[![Crates.io](https://img.shields.io/crates/v/claude-ime.svg)](https://crates.io/crates/claude-ime)
[![npm](https://img.shields.io/npm/v/@agenon/claude-ime.svg)](https://www.npmjs.com/package/@agenon/claude-ime)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## The Problem

Claude Code's terminal doesn't handle IME (Input Method Editor) composition correctly. When you type Vietnamese (Unikey, fcitx), Chinese (Sogou, fcitx), Japanese (Mozc), or Korean (fcitx-hangul), characters get garbled or split across multiple lines because the PTY doesn't respect UTF-8 character boundaries during IME composition.

This breaks your workflow for the 8–10 million developers worldwide who rely on non-Latin input methods.

## The Fix

**claude-ime** wraps Claude Code in a pseudo-terminal (PTY) with proper UTF-8 boundary handling. It:

- Detects IME composition sequences
- Buffers incomplete multibyte characters
- Flushes only when a complete UTF-8 character arrives
- Preserves raw terminal mode for unbroken input/output

Result: seamless IME typing in Claude Code.

## Quick Install

### Option 1: Cargo (Rust)

```bash
cargo install claude-ime
```

### Option 2: npm

```bash
npm install -g @agenon/claude-ime
```

The npm wrapper auto-downloads the precompiled binary for your platform (darwin/linux/win32, x64/arm64).

### Option 3: Manual Binary Download

Download a precompiled binary from [GitHub Releases](https://github.com/agenon/claude-ime/releases), extract, and add to `$PATH`:

```bash
# Example: macOS ARM64
curl -L https://github.com/agenon/claude-ime/releases/download/v0.2.1/claude-ime-aarch64-apple-darwin.tar.gz | tar xz -C /usr/local/bin
chmod +x /usr/local/bin/claude-ime
```

## Usage

### Wrap Claude Code

```bash
claude-ime
```

This auto-detects the claude binary and wraps it. Now type normally in Vietnamese, Chinese, Japanese, or Korean—no garbling.

### Wrap Any Command

```bash
claude-ime --wrap echo "Hello"
```

### Debug Output

```bash
claude-ime -d
```

Shows PTY setup, IME detection, and UTF-8 boundary crossing events.

### Version

```bash
claude-ime -v
```

## How It Works

```
Your keyboard (IME)
       ↓
  PTY wrapper (claude-ime)
       ├─ Raw mode + UTF-8 handling
       ├─ Composition sequence detection
       ├─ Multibyte character buffering
       └─ Bidirectional proxy
       ↓
  Claude Code terminal
```

1. **PTY Spawn**: Claude Code runs inside a pseudo-terminal, not direct pipes.
2. **Raw Mode**: Input/output bypass line-buffering to preserve IME sequences.
3. **UTF-8 Boundary Detection**: Incoming bytes are scanned for complete UTF-8 characters.
4. **Composition Proxy**: Incomplete sequences (IME composition) are held; complete characters are flushed immediately.
5. **Bidirectional**: Stdin → Claude, Claude output → stdout (transparent pass-through).

## Supported IMEs

| Language   | IME Engines                          | Status |
|------------|--------------------------------------|--------|
| Vietnamese | Unikey, fcitx-unikey, macOS         | ✓      |
| Chinese    | Sogou, fcitx-pinyin, macOS Pinyin   | ✓      |
| Japanese   | Mozc, fcitx-mozc, macOS Japanese    | ✓      |
| Korean     | fcitx-hangul, macOS Korean          | ✓      |

Other IMEs (Thai, Lao, Khmer, Arabic, etc.) may work; report issues.

## Configuration

Create `~/.config/claude-ime/config.toml`:

```toml
# Path to the claude binary (auto-detected if not set)
# claude_path = "/usr/local/bin/claude"

# Enable verbose logging (same as --verbose flag)
# verbose = false

# Extra environment variables to pass to the wrapped command
# [extra_env]
# LANG = "en_US.UTF-8"
# LC_ALL = "en_US.UTF-8"
```

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for:
- Build instructions (`cargo build`, `cargo test`)
- Testing with different IMEs
- PR process
- Code style (cargo fmt, cargo clippy)

## License

MIT License. See [LICENSE](LICENSE) for details.

---

**Built by [Agenon](https://agenon.ai)** — AI-operated software.
