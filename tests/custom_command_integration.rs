#![cfg(unix)]

// Integration test for terminal setup and restoration after custom commands.
//
// Why this structure:
// - libtest captures stdout/stderr, which hides alt-screen escape codes.
// - we run a helper binary (terminal_probe) under a PTY so its stdio is a real TTY.
// - terminal_probe reports results via stdout markers; panics on failure.
// - the PTY master captures escape sequences and status markers.
// - we use non-blocking reads to avoid hanging on PTY EOF.
use std::ffi::CStr;
use std::io::Read;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::process::{Command, Stdio};

/// Format bytes as an escaped string for diagnostic output.
fn escape_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes.iter().take(500) {
        match b {
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x1b => out.push_str("\\x1b"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out
}

/// Open a PTY pair, returning (master file, slave fd).
fn open_pty() -> (std::fs::File, i32) {
    let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    assert!(master_fd >= 0, "Failed to open PTY");
    assert!(unsafe { libc::grantpt(master_fd) } == 0, "grantpt failed");
    assert!(
        unsafe { libc::unlockpt(master_fd) } == 0,
        "unlockpt failed"
    );
    let slave_name = unsafe { libc::ptsname(master_fd) };
    assert!(!slave_name.is_null(), "ptsname failed");
    let slave_fd = unsafe {
        libc::open(
            CStr::from_ptr(slave_name).as_ptr(),
            libc::O_RDWR | libc::O_NOCTTY,
        )
    };
    assert!(slave_fd >= 0, "Failed to open PTY slave");
    let master = unsafe { std::fs::File::from_raw_fd(master_fd) };
    (master, slave_fd)
}

/// Test that we return to alternate screen and raw mode after a custom command.
#[test]
fn custom_command_restores_tui_screen_and_raw_mode() {
    let (mut master, slave_fd) = open_pty();

    // Spawn the helper binary with the PTY as its stdio.
    // Each Stdio::from_raw_fd takes ownership, so dup for stdout/stderr
    // and let stdin consume slave_fd directly (last use).
    let stdout_fd = unsafe { libc::dup(slave_fd) };
    assert!(stdout_fd >= 0, "Failed to dup PTY for stdout");
    let stderr_fd = unsafe { libc::dup(slave_fd) };
    assert!(stderr_fd >= 0, "Failed to dup PTY for stderr");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_terminal_probe"));
    unsafe {
        cmd.stdin(Stdio::from_raw_fd(slave_fd));
        cmd.stdout(Stdio::from_raw_fd(stdout_fd));
        cmd.stderr(Stdio::from_raw_fd(stderr_fd));
    }
    let mut child = cmd.spawn().expect("Failed to spawn terminal_probe");
    let status = child.wait().expect("Failed to wait for child");

    // Read PTY output (non-blocking) before asserting so we always have diagnostics.
    let master_fd = master.as_raw_fd();
    unsafe {
        let flags = libc::fcntl(master_fd, libc::F_GETFL);
        libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let mut output = Vec::new();
    let mut buf = [0u8; 1024];
    for _ in 0..50 {
        match master.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => output.extend_from_slice(&buf[..n]),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(err) => panic!("Failed reading PTY: {err}"),
        }
    }
    let output = String::from_utf8_lossy(&output);
    let escaped = escape_bytes(output.as_bytes());

    // terminal_probe should run to completion. If run_command() fails it will
    // panic, and the panic message will appear in the PTY output below.
    assert!(
        status.success(),
        "terminal_probe failed: {status}\nPTY output: {escaped}"
    );

    // run_command() should disable raw mode while the command runs.
    assert!(
        output.contains("COOKED_DURING_CMD"),
        "raw mode was not disabled during command execution\nPTY output: {escaped}",
    );

    // run_command() should restore raw mode afterwards.
    assert!(
        output.contains("RAW_MODE:true"),
        "raw mode not restored after command execution\nPTY output: {escaped}",
    );

    // Extract the region between SUT_START and SUT_END to inspect only
    // escape sequences produced by run_command(), ignoring the fixture
    // (ratatui::init / ratatui::restore).
    let sut_start = output.find("SUT_START").expect("Missing SUT_START marker");
    let sut_end = output.find("SUT_END").expect("Missing SUT_END marker");
    let sut_output = &output[sut_start..sut_end];

    // run_command() should re-enter alternate screen.
    assert!(
        sut_output.contains("\x1b[?1049h"),
        "did not re-enter alternate screen\nSUT output: {}",
        escape_bytes(sut_output.as_bytes()),
    );
}
