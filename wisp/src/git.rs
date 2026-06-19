use anyhow::Result;
use std::process::Command;

pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn working_tree_clean() -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    Ok(output.stdout.is_empty())
}

pub fn current_branch() -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Some(branch))
    } else {
        Ok(None)
    }
}

pub fn diff() -> Result<String> {
    let output = Command::new("git").args(["diff"]).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn status() -> Result<String> {
    let output = Command::new("git").args(["status"]).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
