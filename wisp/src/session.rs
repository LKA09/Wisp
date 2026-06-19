use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Session {
    pub dir: PathBuf,
}

impl Session {
    /// Create a new timestamped session directory under .wisp/sessions/.
    pub fn create() -> Result<Self> {
        let now = Local::now();
        let pid = std::process::id();
        let timestamp = format!(
            "{}-{:03}-p{}",
            now.format("%Y%m%d-%H%M%S"),
            now.timestamp_subsec_millis(),
            pid
        );
        let dir = PathBuf::from(".wisp/sessions").join(&timestamp);

        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create session directory: {}", dir.display()))?;
        fs::create_dir_all(dir.join("prompts")).context("Failed to create prompts directory")?;
        fs::create_dir_all(dir.join("outputs")).context("Failed to create outputs directory")?;
        fs::create_dir_all(dir.join("git")).context("Failed to create git directory")?;

        Ok(Session { dir })
    }

    /// Write a file relative to the session directory.
    pub fn write(&self, relative_path: &str, content: &str) -> Result<()> {
        let path = self.dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)
            .with_context(|| format!("Failed to write session file: {}", path.display()))?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }
}
