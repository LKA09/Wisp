use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Persistent per-project settings stored in `.wisp/settings.toml`.
/// Distinct from `wisp.toml` (agent/workflow configuration).
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Settings {
    /// When true, bare tasks invoke agents; when false (default), show dry-run preview.
    #[serde(default)]
    pub execute_agents: bool,
}

impl Settings {
    const PATH: &'static str = ".wisp/settings.toml";

    pub fn load() -> Self {
        std::fs::read_to_string(Self::PATH)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(Self::PATH, content)?;
        Ok(())
    }
}
