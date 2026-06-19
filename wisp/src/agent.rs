use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

fn spawn_cmd(cmd: &str, args: &[String], prompt: &str, cwd: &Path) -> Result<Child> {
    use std::io::Write;

    #[cfg(windows)]
    let mut child = Command::new("cmd")
        .arg("/c")
        .arg(cmd)
        .args(args)
        .arg("-")
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

pub struct SubprocessRunner {
    pub config: AgentConfig,
}

enum StreamEvent {
    Stdout(String),
    Stderr(String),
}

impl SubprocessRunner {
    pub fn run_streaming<F: FnMut(&str)>(
        &self,
        prompt: &str,
        cwd: &Path,
        mut on_chunk: F,
    ) -> Result<AgentOutput> {
        use std::io::{BufRead, BufReader, Read};

        let mut child = spawn_cmd(&self.config.cmd, &self.config.args, prompt, cwd)?;
        let stdout = child.stdout.take().context("stdout pipe missing")?;
        let stderr = child.stderr.take().context("stderr pipe missing")?;

        let (tx, rx) = mpsc::channel::<StreamEvent>();

        let stdout_tx = tx.clone();
        let stdout_thread = thread::spawn(move || -> Result<()> {
            for line in BufReader::new(stdout).lines() {
                let line = line?;
                let mut chunk = line;
                chunk.push('\n');
                if stdout_tx.send(StreamEvent::Stdout(chunk)).is_err() {
                    break;
                }
            }
            Ok(())
        });

        let stderr_thread = thread::spawn(move || -> Result<()> {
            let mut collected = String::new();
            BufReader::new(stderr).read_to_string(&mut collected)?;
            let _ = tx.send(StreamEvent::Stderr(collected));
            Ok(())
        });

        let mut all_stdout = String::new();
        let mut all_stderr = String::new();
        for event in rx {
            match event {
                StreamEvent::Stdout(chunk) => {
                    on_chunk(&chunk);
                    all_stdout.push_str(&chunk);
                }
                StreamEvent::Stderr(chunk) => {
                    all_stderr.push_str(&chunk);
                }
            }
        }

        stdout_thread
            .join()
            .map_err(|_| anyhow::anyhow!("stdout reader thread panicked"))??;
        stderr_thread
            .join()
            .map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))??;

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

pub struct DryRunRunner {
    pub config: AgentConfig,
}

impl DryRunRunner {
    pub fn display_and_capture(&self, prompt: &str) -> AgentOutput {
        use crate::display;

        display::agent_line(&format!(
            "\x1b[2m\x1b[90m[dry-run]\x1b[0m  {} {}  <prompt>",
            self.config.cmd,
            self.config.args.join(" "),
        ));
        display::agent_blank();

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
