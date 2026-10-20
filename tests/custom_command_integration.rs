#![cfg(unix)]

// Integration test for terminal setup and restoration after custom commands.
//
// Why this structure:
// - libtest captures stdout/stderr, which hides alt-screen escape codes.
// - we run a helper binary (terminal_probe) under a PTY so its stdio is a real TTY.
// - terminal_probe reports results via stdout markers; panics on failure.
// - the PTY master captures output and answers cursor-position queries, since
//   a PTY alone does not emulate a terminal.
// - we use non-blocking reads and a deadline to avoid hanging.
use std::ffi::CStr;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
    assert!(unsafe { libc::unlockpt(master_fd) } == 0, "unlockpt failed");
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
    // Close the parent's slave handles so EOF is visible when the helper exits.
    drop(cmd);

    let master_fd = master.as_raw_fd();
    let flags = unsafe { libc::fcntl(master_fd, libc::F_GETFL) };
    assert!(flags >= 0, "Failed to get PTY flags");
    assert_eq!(
        unsafe { libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0,
        "Failed to make PTY non-blocking",
    );

    // Read while the helper runs: Terminal::clear() queries the cursor position
    // and waits for a reply. Waiting for the child first would cause a timeout.
    const CURSOR_QUERY: &[u8] = b"\x1b[6n";
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut output = Vec::new();
    let mut scan_from = 0;
    let mut buf = [0u8; 1024];
    let status = loop {
        let read_output = match master.read(&mut buf) {
            Ok(0) => false,
            Ok(n) => {
                output.extend_from_slice(&buf[..n]);
                for window in output[scan_from..].windows(CURSOR_QUERY.len()) {
                    if window == CURSOR_QUERY {
                        // A fixed, non-origin position is enough for this test.
                        // Using (1, 1) would let clear()'s cursor restoration mask
                        // a missing explicit MoveTo(0, 0) in run_command().
                        master
                            .write_all(b"\x1b[5;9R")
                            .expect("Failed to reply to cursor query");
                    }
                }
                // Retain a possible query prefix split across consecutive reads.
                scan_from = output.len().saturating_sub(CURSOR_QUERY.len() - 1);
                true
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => false,
            // Linux PTYs report EIO rather than EOF when the slave closes.
            Err(err) if err.raw_os_error() == Some(libc::EIO) => false,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => panic!("Failed reading PTY: {err}"),
        };
        // Drain all pending output before asserting, even if the helper failed.
        if let Some(status) = child.try_wait().expect("Failed to poll child")
            && !read_output
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "terminal_probe timed out\nPTY output: {}",
                escape_bytes(&output),
            );
        }
        if !read_output {
            std::thread::sleep(Duration::from_millis(10));
        }
    };
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

    // run_command() should move cursor to top-left before running the command.
    assert!(
        sut_output.contains("\x1b[1;1H"),
        "did not move cursor to top-left\nSUT output: {}",
        escape_bytes(sut_output.as_bytes()),
    );

    // run_command() should re-enter alternate screen.
    assert!(
        sut_output.contains("\x1b[?1049h"),
        "did not re-enter alternate screen\nSUT output: {}",
        escape_bytes(sut_output.as_bytes()),
    );
}
