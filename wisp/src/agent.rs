use anyhow::Result;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Spawn an agent subprocess with the prompt fed via stdin.
/// Stdin avoids Windows quoting issues with long, multi-line prompts.
/// On Windows, npm CLIs are .cmd files and must run through `cmd /c`.
fn spawn_cmd(cmd: &str, args: &[String], prompt: &str, cwd: &Path) -> Result<Child> {
    use std::io::Write;

    #[cfg(windows)]
    let mut child = Command::new("cmd")
        .arg("/c")
        .arg(cmd)
        .args(args)
        .arg("-")          // tell the CLI to read prompt from stdin
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    #[cfg(not(windows))]
    let mut child = Command::new(cmd)
        .args(args)
        .arg("-")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Write prompt to stdin and close it so the CLI knows input is done.
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
    }

    Ok(child)
}

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

/// Runs the agent as a subprocess, streaming each output line via a callback.
pub struct SubprocessRunner {
    pub config: AgentConfig,
}

impl SubprocessRunner {
    pub fn run_streaming<F: FnMut(&str)>(
        &self,
        prompt: &str,
        cwd: &Path,
        mut on_line: F,
    ) -> Result<AgentOutput> {
        use std::io::{BufRead, BufReader, Read};

        let mut child = spawn_cmd(&self.config.cmd, &self.config.args, prompt, cwd)?;

        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let mut all_stdout = String::new();
        for line in BufReader::new(stdout).lines() {
            let line = line?;
            on_line(&line);
            all_stdout.push_str(&line);
            all_stdout.push('\n');
        }

        let mut all_stderr = String::new();
        BufReader::new(stderr).read_to_string(&mut all_stderr)?;

        let status = child.wait()?;

        Ok(AgentOutput {
            status: status.code().unwrap_or(-1),
            stdout: all_stdout,
            stderr: all_stderr,
        })
    }
}

impl AgentRunner for SubprocessRunner {
    fn run(&self, prompt: &str, cwd: &Path) -> Result<AgentOutput> {
        self.run_streaming(prompt, cwd, |_| {})
    }
}


/// Dry-run: shows prompt preview in the conversation UI instead of invoking the agent.
pub struct DryRunRunner {
    pub config: AgentConfig,
}

impl DryRunRunner {
    /// Print prompt preview to the conversation UI, return the content for session log.
    pub fn display_and_capture(&self, prompt: &str) -> AgentOutput {
        use crate::display;

        display::agent_line(&format!(
            "\x1b[2m\x1b[90m[dry-run]\x1b[0m  {} {}  <prompt>",
            self.config.cmd,
            self.config.args.join(" "),
        ));
        display::agent_blank();

        // Show first 12 lines of the prompt
        let lines: Vec<&str> = prompt.lines().collect();
        let preview_count = lines.len().min(12);
        for line in &lines[..preview_count] {
            display::agent_line(line);
        }
        if lines.len() > preview_count {
            display::agent_blank();
            display::agent_line(&format!(
                "\x1b[90m... {} chars total. Full prompt in session prompts/\x1b[0m",
                prompt.len()
            ));
        }

        AgentOutput {
            status: 0,
            stdout: format!(
                "[dry-run] Would invoke: {} {}\nPrompt ({} chars):\n---\n{}\n---\n",
                self.config.cmd,
                self.config.args.join(" "),
                prompt.len(),
                prompt,
            ),
            stderr: String::new(),
        }
    }
}

impl AgentRunner for DryRunRunner {
    fn run(&self, prompt: &str, _cwd: &Path) -> Result<AgentOutput> {
        Ok(self.display_and_capture(prompt))
    }
}
