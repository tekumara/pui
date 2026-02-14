use anyhow::Result;

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
