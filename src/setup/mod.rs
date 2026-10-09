pub mod agents;
pub mod detect;
pub mod install;
pub mod write;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

fn in_work_tree(dir: &Path) -> bool {
    git(dir, &["rev-parse", "--is-inside-work-tree"]).is_ok()
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir).args(args);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let output = command.output().map_err(|e| format!("git: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
