use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum PermissionMode {
    Interactive,
    Auto,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentInputMode {
    PromptViaStdinClosed,
    PromptViaTempFile,
    PromptViaArgs,
    InteractiveStdin,
}

#[derive(Debug, Clone)]
pub struct AgentRunOptions {
    pub permission_mode: PermissionMode,
    pub input_mode: AgentInputMode,
    pub capture_output: bool,
    pub stream_output: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedAgentCommand {
    pub cmd: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub struct AgentOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct SubprocessRunner {
    pub options: AgentRunOptions,
}

pub struct DryRunRunner {
    pub options: AgentRunOptions,
}

enum StreamEvent {
    Stdout(String),
    Stderr(String),
}

fn permission_args(
    permission_mode: PermissionMode,
    interactive: &[String],
    auto: &[String],
    skip: &[String],
) -> Vec<String> {
    match permission_mode {
        PermissionMode::Interactive => interactive.to_vec(),
        PermissionMode::Auto => auto.to_vec(),
        PermissionMode::Skip => skip.to_vec(),
    }
}

pub fn resolve_input_mode(input: &str) -> AgentInputMode {
    match input {
        "file" => AgentInputMode::PromptViaTempFile,
        "stdin" => AgentInputMode::PromptViaStdinClosed,
        "interactive-stdin" => AgentInputMode::InteractiveStdin,
        _ => AgentInputMode::PromptViaArgs,
    }
}

pub fn prepare_command(
    config: &crate::config::AgentConfig,
    _name: &str,
    task: &str,
    session_dir: &Path,
    prompt: &str,
    prompt_file: &Path,
    permission_mode: PermissionMode,
) -> PreparedAgentCommand {
    let mut vars = HashMap::new();
    vars.insert("prompt".to_string(), prompt.to_string());
    vars.insert(
        "prompt_file".to_string(),
        prompt_file.to_string_lossy().to_string(),
    );
    vars.insert(
        "session_dir".to_string(),
        session_dir.to_string_lossy().to_string(),
    );
    vars.insert("task".to_string(), task.to_string());

    let mut args = config.args.clone();
    args.extend(permission_args(
        permission_mode,
        &config.permission_interactive_args,
        &config.permission_auto_args,
        &config.permission_skip_args,
    ));

    let resolved_args = args
        .iter()
        .map(|arg| substitute_placeholders(arg, &vars))
        .collect::<Vec<_>>();
    PreparedAgentCommand {
        cmd: config.cmd.clone(),
        args: resolved_args,
    }
}

fn substitute_placeholders(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}

fn spawn_cmd(cmd: &str, args: &[String], cwd: &Path, options: &AgentRunOptions) -> Result<Child> {
    let mut command = Command::new(cmd);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(match options.input_mode {
            AgentInputMode::PromptViaStdinClosed => Stdio::piped(),
            AgentInputMode::PromptViaTempFile
            | AgentInputMode::PromptViaArgs
            | AgentInputMode::InteractiveStdin => Stdio::inherit(),
        });

    if options.capture_output {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }

    Ok(command.spawn()?)
}

impl SubprocessRunner {
    pub fn run_streaming<F: FnMut(&str)>(
        &self,
        prepared: &PreparedAgentCommand,
        cwd: &Path,
        mut on_chunk: F,
    ) -> Result<AgentOutput> {
        use std::io::{BufRead, BufReader, Read};

        let mut child = spawn_cmd(&prepared.cmd, &prepared.args, cwd, &self.options)?;

        if !self.options.capture_output {
            let status = child.wait()?;
            return Ok(AgentOutput {
                status: status.code().unwrap_or(-1),
                stdout: String::new(),
                stderr: String::new(),
            });
        }

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
                    if self.options.stream_output {
                        on_chunk(&chunk);
                    }
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

impl DryRunRunner {
    pub fn display_and_capture(
        &self,
        prepared: &PreparedAgentCommand,
        prompt: &str,
    ) -> AgentOutput {
        use crate::display;

        display::agent_line(&format!(
            "\x1b[2m\x1b[90m[dry-run]\x1b[0m  {} {}",
            prepared.cmd,
            prepared.args.join(" "),
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
                "[dry-run] Would invoke: {} {}\nPermission mode: {:?}\nPrompt ({} chars):\n---\n{}\n---\n",
                prepared.cmd,
                prepared.args.join(" "),
                self.options.permission_mode,
                prompt.len(),
                prompt,
            ),
            stderr: String::new(),
        }
    }
}
