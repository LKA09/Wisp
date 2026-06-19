use anyhow::Result;
use std::collections::BTreeMap;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub index_status: char,
    pub worktree_status: char,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct GitSnapshot {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub status_raw: String,
    pub status_entries: Vec<StatusEntry>,
    pub diff_name_status: String,
}

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
    Ok(status_porcelain()?.trim().is_empty())
}

pub fn current_branch() -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    if output.status.success() {
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    } else {
        Ok(None)
    }
}

pub fn current_head() -> Result<Option<String>> {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    if output.status.success() {
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
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

pub fn status_porcelain() -> Result<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1"])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn diff_name_status() -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "--name-status", "--find-renames"])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn snapshot() -> Result<GitSnapshot> {
    let status_raw = status_porcelain()?;
    let status_entries = parse_status_entries(&status_raw);
    Ok(GitSnapshot {
        branch: current_branch()?,
        head: current_head()?,
        status_raw,
        status_entries,
        diff_name_status: diff_name_status()?,
    })
}

pub fn delta_status_entries(before: &GitSnapshot, after: &GitSnapshot) -> Vec<StatusEntry> {
    let before_map: BTreeMap<String, (char, char)> = before
        .status_entries
        .iter()
        .map(|entry| {
            (
                entry.path.clone(),
                (entry.index_status, entry.worktree_status),
            )
        })
        .collect();

    after
        .status_entries
        .iter()
        .filter(|entry| {
            before_map
                .get(&entry.path)
                .map(|statuses| *statuses != (entry.index_status, entry.worktree_status))
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

fn parse_status_entries(status_raw: &str) -> Vec<StatusEntry> {
    status_raw.lines().filter_map(parse_status_line).collect()
}

fn parse_status_line(line: &str) -> Option<StatusEntry> {
    if line.len() < 4 {
        return None;
    }

    let index_status = line.chars().next()?;
    let worktree_status = line.chars().nth(1)?;
    let raw_path = line.get(3..)?.trim();
    let path = raw_path
        .split(" -> ")
        .last()
        .unwrap_or(raw_path)
        .trim()
        .to_string();

    Some(StatusEntry {
        index_status,
        worktree_status,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_status_entries;

    #[test]
    fn parses_basic_status_entries() {
        let entries = parse_status_entries(" M src/main.rs\nA  Cargo.lock\n");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].worktree_status, 'M');
        assert_eq!(entries[0].path, "src/main.rs");
        assert_eq!(entries[1].index_status, 'A');
        assert_eq!(entries[1].path, "Cargo.lock");
    }

    #[test]
    fn parses_rename_target_path() {
        let entries = parse_status_entries("R  old.txt -> new.txt\n");
        assert_eq!(entries[0].path, "new.txt");
    }
}
