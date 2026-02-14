use anyhow::Result;
use ratatui::DefaultTerminal;

pub fn run_command_internal(cmd: &[String], working_dir: &std::path::Path) -> Result<()> {
    // Run the command
    let result = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(working_dir)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match result {
        Ok(status) => {
            if status.success() {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "Command exited with status: {}",
                    status.code().unwrap_or(-1)
                ))
            }
        }
        Err(e) => Err(e.into()),
    }
}

/// Run a command, temporarily suspending TUI mode.
/// Disables raw mode so the command can use normal terminal I/O,
/// then restores raw mode when done.
pub fn run_command(
    terminal: &mut DefaultTerminal,
    cmd: &[String],
    working_dir: &std::path::Path,
) -> Result<()> {
    terminal.clear()?;
    crossterm::terminal::disable_raw_mode()?;

    let result = run_command_internal(cmd, working_dir);

    // Re-enter alternate screen and enable raw mode for the TUI.
    crossterm::terminal::enable_raw_mode()?;
    terminal.clear()?;

    result
}
