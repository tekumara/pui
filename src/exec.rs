use anyhow::Result;
use crossterm::terminal::EnterAlternateScreen;
use ratatui::DefaultTerminal;

pub fn spawn_process(cmd: &[String], working_dir: &std::path::Path) -> Result<()> {
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

/// Run a command, temporarily leaving TUI mode, and re-enabling it afterwards
pub fn run_command(
    terminal: &mut DefaultTerminal,
    cmd: &[String],
    working_dir: &std::path::Path,
) -> Result<()> {
    // Clear the screen immediately so the user doesn't see TUI remnants while the command runs
    terminal.clear()?;

    // Move cursor to top-left so the command's output (eg: bash) starts on first line, and
    // not were the cursor is at the time (ie: on the row in the tasks table)
    crossterm::execute!(std::io::stdout(), crossterm::cursor::MoveTo(0, 0))?;

    // NB: we deliberately stay in the alternate screen, rather than leaving it, to avoid
    // polluting the main screen and terminal buffer (we could clear it, but that would leave a blank hole)

    // Disable raw mode so the external command runs in a normal terminal environment
    // (input is line-buffered, typed characters echo, Ctrl+C sends SIGINT, etc.)
    crossterm::terminal::disable_raw_mode()?;

    let result = spawn_process(cmd, working_dir);

    // Ratatui expects raw mode to be enabled, so we re-enable it here
    crossterm::terminal::enable_raw_mode()?;

    // Clear the screen immediately, otherwise we may have still have changes on-screen from the command
    terminal.clear()?;

    // Re-enter alternate screen, in case the command (eg: lazygit/tuicr) has left it.
    // Ensures PgDn/PgUp is sent to our TUI and doesn't scroll back the terminal
    crossterm::execute!(std::io::stdout(), EnterAlternateScreen)?;

    result
}
