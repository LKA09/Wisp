use crate::config::Config;
use std::fs;
use std::path::Path;

pub struct LoadedInstructions {
    pub files: Vec<(String, String)>, // (path, content)
    pub total_bytes: usize,
    pub truncated: bool,
}

impl LoadedInstructions {
    /// Combine all loaded instruction files into a single Markdown string.
    pub fn combined(&self) -> String {
        if self.files.is_empty() {
            return "(No project instruction files found.)".to_string();
        }

        let mut result = String::new();
        for (path, content) in &self.files {
            result.push_str(&format!("## {}\n\n{}\n\n", path, content));
        }
        if self.truncated {
            result.push_str("_[Instructions truncated due to max_bytes limit]_\n");
        }
        result
    }
}

/// Load project instruction files as configured in wisp.toml.
pub fn load_instructions(config: &Config) -> LoadedInstructions {
    let mut files = Vec::new();
    let mut total_bytes = 0usize;
    let mut truncated = false;
    let max_bytes = config.instructions.max_bytes;

    let files_to_check: Vec<&String> = if config.instructions.include_agent_specific {
        config.instructions.files.iter().collect()
    } else {
        config
            .instructions
            .files
            .iter()
            .filter(|f| !f.to_uppercase().contains("CLAUDE") && !f.to_uppercase().contains("CODEX"))
            .collect()
    };

    for file_path in files_to_check {
        if total_bytes >= max_bytes {
            truncated = true;
            break;
        }

        let path = Path::new(file_path);
        if !path.exists() {
            continue;
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                let remaining = max_bytes.saturating_sub(total_bytes);
                let (loaded, was_truncated) = if content.len() > remaining {
                    truncated = true;
                    (content[..remaining].to_string(), true)
                } else {
                    (content, false)
                };
                let _ = was_truncated;
                total_bytes += loaded.len();
                files.push((file_path.clone(), loaded));
            }
            Err(_) => continue,
        }
    }

    LoadedInstructions {
        files,
        total_bytes,
        truncated,
    }
}
