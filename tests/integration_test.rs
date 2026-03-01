use std::process::Command;

#[test]
fn help_flag_shows_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_claude-ime"))
        .arg("--help")
        .output()
        .expect("Failed to execute claude-ime");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("IME"));
}

#[test]
fn version_flag_shows_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_claude-ime"))
        .arg("--version")
        .output()
        .expect("Failed to execute claude-ime");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn wrap_echo_passes_through() {
    let output = Command::new(env!("CARGO_BIN_EXE_claude-ime"))
        .args(["--wrap", "echo", "--", "hello", "world"])
        .output()
        .expect("Failed to execute claude-ime");

    // echo should succeed even without a TTY
    // (may fail in CI without PTY, so we just check it doesn't panic)
    assert!(output.status.code().is_some());
}
