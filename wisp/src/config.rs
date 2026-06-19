use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub language: LanguageConfig,
    pub agents: HashMap<String, AgentConfig>,
    pub workflow: WorkflowConfig,
    pub approval: ApprovalConfig,
    pub instructions: InstructionsConfig,
    pub policy: PolicyConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LanguageConfig {
    pub ui: String,
    pub fallback: String,
    pub internal: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AgentConfig {
    pub cmd: String,
    pub args: Vec<String>,
    #[serde(default = "default_agent_input")]
    pub input: String,
    #[serde(default)]
    pub permission_interactive_args: Vec<String>,
    #[serde(default)]
    pub permission_auto_args: Vec<String>,
    #[serde(default)]
    pub permission_skip_args: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkflowConfig {
    pub implementer: String,
    pub patcher: String,
    pub reviewer: String,
    pub shipper: String,
    pub max_review_rounds: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApprovalConfig {
    pub push: String,
    pub commit: String,
    pub add_dependency: String,
    pub delete_file: String,
    pub modify_protected_file: String,
    pub continue_after_test_failure: String,
}

fn default_agent_input() -> String {
    "arg".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InstructionsConfig {
    pub files: Vec<String>,
    pub max_bytes: usize,
    pub include_agent_specific: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PolicyConfig {
    pub protected_branches: Vec<String>,
    pub protected_paths: Vec<String>,
    pub deny_commands: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let content = std::fs::read_to_string("wisp.toml").context("Failed to read wisp.toml")?;
        let config: Config = toml::from_str(&content).context("Failed to parse wisp.toml")?;
        Ok(config)
    }

    pub fn exists() -> bool {
        Path::new("wisp.toml").exists()
    }
}

pub fn default_config_toml() -> &'static str {
    r#"[language]
ui = "auto"
fallback = "en"
internal = "en"

[agents.claude]
cmd = "claude"
args = ["-p", "{prompt}"]
input = "arg"
permission_interactive_args = []
permission_auto_args = []
permission_skip_args = []

[agents.codex]
cmd = "codex"
args = ["exec", "-s", "workspace-write", "{prompt}"]
input = "arg"
permission_interactive_args = []
permission_auto_args = []
permission_skip_args = []

[workflow]
implementer = "claude"
patcher = "codex"
reviewer = "claude"
shipper = "codex"
max_review_rounds = 2

[approval]
push = "deny"
commit = "ask"
add_dependency = "ask"
delete_file = "ask"
modify_protected_file = "deny"
continue_after_test_failure = "ask"

[instructions]
files = [
  ".wisp/instructions.md",
  "WISP.md",
  "AGENTS.md",
  "AGENT.md",
  "CLAUDE.md",
  "CODEX.md"
]
max_bytes = 32768
include_agent_specific = true

[policy]
protected_branches = ["main", "master"]
protected_paths = [".env", ".env.local", ".git", "id_rsa", "secrets.toml", "credentials.json"]
deny_commands = ["git push --force", "cargo publish", "npm publish", "rm -rf /"]
"#
}
