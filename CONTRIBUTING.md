# Contributing to claude-ime

Thank you for your interest in contributing! This guide covers building, testing, and submitting changes.

## Build Instructions

### Prerequisites

- Rust 1.56+ ([install](https://rustup.rs/))
- Cargo

### Build from Source

```bash
git clone https://github.com/agenon/claude-ime.git
cd claude-ime
cargo build --release
```

The binary will be at `target/release/claude-ime` (or `claude-ime.exe` on Windows).

### Run Tests

```bash
cargo test
```

All tests must pass before submitting a PR.

## Code Style

### Format Code

```bash
cargo fmt
```

All code must be formatted with `cargo fmt` before submission.

### Lint Code

```bash
cargo clippy
```

Address all warnings reported by `cargo clippy`. Use `#[allow(...)]` only when justified with an inline comment.

## Testing IMEs

When adding support for new IMEs or fixing composition bugs, test with real IMEs:

### macOS

1. **Enable language**: System Preferences → Keyboard → Input Sources → Add (e.g., Vietnamese, Simplified Chinese)
2. **Switch input source**: Cmd+Space → select language
3. **Run claude-ime**:
   ```bash
   cargo run -- --verbose
   ```
4. **Type in the target language**: Verify no garbling, characters appear correctly
5. **Check logs**: Watch for UTF-8 boundary crossings in verbose output

### Linux (fcitx)

1. **Install fcitx and IME** (example: Unikey for Vietnamese):
   ```bash
   sudo apt install fcitx fcitx-unikey
   ```
2. **Configure fcitx**: `fcitx-configtool`
3. **Set input method**: `XIM=fcitx XMODIFIERS=@im=fcitx claude-ime --verbose`
4. **Type and verify**

### Windows (IME Support)

1. **Enable IME**: Settings → Devices → Typing → Input method
2. **Run claude-ime**:
   ```bash
   cargo run -- --verbose
   ```
3. **Type** and verify UTF-8 handling

## PR Process

1. **Fork** the repository
2. **Create a branch**: `git checkout -b fix/your-issue` or `feat/your-feature`
3. **Make changes** and test locally
4. **Run checks**:
   ```bash
   cargo fmt
   cargo clippy
   cargo test
   ```
5. **Commit** with a clear message:
   ```
   feat: support new IME composition mode
   fix: handle incomplete UTF-8 sequences
   ```
6. **Push** and open a PR with:
   - Clear description of the change
   - Link to related issue (if any)
   - Testing steps for IME fixes
   - Platform(s) tested on

## Issue Templates

### Bug Report

```
**Platform**: macOS / Linux / Windows
**Rust Version**: (output of `rustc --version`)
**IME**: Vietnamese Unikey / Chinese Sogou / etc.

**Describe the bug**:
When I type [description], [what happens].

**Expected behavior**:
[what should happen]

**Steps to reproduce**:
1. Enable IME
2. Run `claude-ime --verbose`
3. Type [example]

**Logs**:
```
[paste verbose output]
```
```

### Feature Request

```
**Title**: Brief feature description

**Use case**: Why do you need this?

**Proposed solution**: How would you like it to work?

**Alternative approaches**: Other ideas?
```

## Questions?

- Open a [GitHub Discussion](https://github.com/agenon/claude-ime/discussions)
- Check [existing issues](https://github.com/agenon/claude-ime/issues)

---

Thank you for improving claude-ime!
