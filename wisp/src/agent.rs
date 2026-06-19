use anyhow::Result;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub cmd: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub struct AgentOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait AgentRunner {
    fn run(&self, prompt: &str, cwd: &Path) -> Result<AgentOutput>;
}

/// Runs the agent as a real subprocess. Used when --execute-agents is passed.
pub struct SubprocessRunner {
    pub config: AgentConfig,
}

impl AgentRunner for SubprocessRunner {
    fn run(&self, prompt: &str, cwd: &Path) -> Result<AgentOutput> {
        let mut cmd = Command::new(&self.config.cmd);
        cmd.args(&self.config.args);
        cmd.arg(prompt);
        cmd.current_dir(cwd);

        let output = cmd.output()?;

        Ok(AgentOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

/// Dry-run: writes placeholder output instead of invoking the agent.
pub struct DryRunRunner {
    pub config: AgentConfig,
}

impl AgentRunner for DryRunRunner {
    fn run(&self, prompt: &str, _cwd: &Path) -> Result<AgentOutput> {
        let preview_len = prompt.len().min(300);
        Ok(AgentOutput {
            status: 0,
            stdout: format!(
                "[DRY RUN] Would execute: {} {}\n\n\
                 Prompt ({} chars) preview:\n---\n{}\n---\n\n\
                 Pass --execute-agents to invoke the real agent.",
                self.config.cmd,
                self.config.args.join(" "),
                prompt.len(),
                &prompt[..preview_len]
            ),
            stderr: String::new(),
        })
    }
}
