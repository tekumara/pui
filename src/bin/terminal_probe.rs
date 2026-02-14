use std::io::Write;
use std::path::Path;

use pui::exec::run_command;

/// Helper binary for integration tests. Runs under a PTY so the test can
/// capture escape sequences. Reports results via stdout markers; panics on
/// failure so the test gets diagnostic output from the PTY.
fn main() {
    // -- Fixture: enter TUI mode (as the real app does) --
    let mut terminal = ratatui::init();

    // SUT_START/SUT_END markers let the test isolate escape sequences
    // produced by run_command() from those produced by the fixture.
    print!("SUT_START");
    std::io::stdout().flush().unwrap();

    // -- System under test --
    // The shell snippet checks that run_command() disabled raw mode before
    // spawning the child. '[^-]icanon' matches icanon without a '-' prefix,
    // meaning canonical (cooked) mode is active. The marker is asserted in the test.
    run_command(
        &mut terminal,
        &[
            "sh".to_string(),
            "-c".to_string(),
            "stty -a 2>/dev/null | grep -q '[^-]icanon' && printf COOKED_DURING_CMD; true"
                .to_string(),
        ],
        Path::new("."),
    )
    .unwrap();

    let raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    print!("SUT_END");
    print!("RAW_MODE:{raw}");
    std::io::stdout().flush().unwrap();

    // -- Fixture: restore terminal --
    ratatui::restore();
}
