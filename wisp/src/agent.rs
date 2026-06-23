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
    pub stdin_payload: Option<String>,
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
    let input_mode = resolve_input_mode(&config.input);
    PreparedAgentCommand {
        cmd: config.cmd.clone(),
        args: resolved_args,
        stdin_payload: matches!(input_mode, AgentInputMode::PromptViaStdinClosed)
            .then(|| prompt.to_string()),
    }
}

fn substitute_placeholders(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}

fn make_command(cmd: &str, args: &[String], cwd: &Path, options: &AgentRunOptions) -> Command {
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
    command
}

fn command_not_found_error(cmd: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "`{cmd}` is not installed or not in PATH\n  \
         -> Install it, or switch to dry-run mode with: wisp mode dry-run"
    )
}

#[cfg(target_os = "windows")]
fn resolve_windows_cmd(cmd: &str) -> Option<String> {
    let escaped = cmd.replace('\'', "''");
    let script = format!(
        "$resolved = Get-Command -Name '{escaped}' -ErrorAction SilentlyContinue | \
         Select-Object -First 1 -ExpandProperty Source; if ($resolved) {{ Write-Output $resolved }}"
    );

    Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
}

#[cfg(target_os = "windows")]
fn spawn_windows_resolved(
    resolved: &str,
    args: &[String],
    cwd: &Path,
    options: &AgentRunOptions,
) -> std::io::Result<Child> {
    let ext = Path::new(resolved)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();

    if ext.eq_ignore_ascii_case("ps1") {
        let mut ps_args = vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            resolved.to_string(),
        ];
        ps_args.extend_from_slice(args);
        make_command("powershell", &ps_args, cwd, options).spawn()
    } else if ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat") {
        let mut cmd_args = vec!["/C".to_string(), resolved.to_string()];
        cmd_args.extend_from_slice(args);
        make_command("cmd", &cmd_args, cwd, options).spawn()
    } else {
        make_command(resolved, args, cwd, options).spawn()
    }
}

fn spawn_cmd(cmd: &str, args: &[String], cwd: &Path, options: &AgentRunOptions) -> Result<Child> {
    let result = make_command(cmd, args, cwd, options).spawn();

    #[cfg(target_os = "windows")]
    if let Err(err) = result {
        if let Some(resolved) = resolve_windows_cmd(cmd) {
            return spawn_windows_resolved(&resolved, args, cwd, options)
                .map_err(anyhow::Error::from);
        }

        return Err(if err.kind() == std::io::ErrorKind::NotFound {
            command_not_found_error(cmd)
        } else {
            anyhow::Error::from(err)
        });
    }

    result.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            command_not_found_error(cmd)
        } else {
            anyhow::Error::from(e)
        }
    })
}

impl SubprocessRunner {
    pub fn run_streaming<F: FnMut(&str)>(
        &self,
        prepared: &PreparedAgentCommand,
        cwd: &Path,
        mut on_chunk: F,
    ) -> Result<AgentOutput> {
        use std::io::{BufRead, BufReader, Read, Write};

        let mut child = spawn_cmd(&prepared.cmd, &prepared.args, cwd, &self.options)?;

        if let Some(payload) = &prepared.stdin_payload {
            let mut stdin = child.stdin.take().context("stdin pipe missing")?;
            stdin.write_all(payload.as_bytes())?;
            drop(stdin);
        }

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
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                let bytes_read = reader.read_until(b'\n', &mut buf)?;
                if bytes_read == 0 {
                    break;
                }
                let chunk = String::from_utf8_lossy(&buf).into_owned();
                if stdout_tx.send(StreamEvent::Stdout(chunk)).is_err() {
                    break;
                }
            }
            Ok(())
        });

        let stderr_thread = thread::spawn(move || -> Result<()> {
            let mut collected = Vec::new();
            BufReader::new(stderr).read_to_end(&mut collected)?;
            let collected = String::from_utf8_lossy(&collected).into_owned();
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
    /// Display a dry-run preview for one agent step.
    ///
    /// Shows the agent name, role, command, prompt path, and a compact
    /// summary of the prompt. The returned `AgentOutput.stdout` is a
    /// neutral marker that does not contain review-decision tokens, so
    /// callers can detect "dry-run preview" without confusing the review
    /// decision parser.
    pub fn display_and_capture(
        &self,
        prepared: &PreparedAgentCommand,
        agent: &str,
        role: &str,
        prompt: &str,
        prompt_path: &std::path::Path,
    ) -> AgentOutput {
        use crate::display;

        let cmd_preview = format!("{} {}", prepared.cmd, prepared.args.join(" "));
        let prompt_char_count = prompt.chars().count();
        let prompt_path_str = prompt_path.display().to_string();

        let permission_label = match self.options.permission_mode {
            PermissionMode::Interactive => "interactive",
            PermissionMode::Auto => "auto",
            PermissionMode::Skip => "skip",
        };
        let input_label = match self.options.input_mode {
            AgentInputMode::PromptViaArgs => "arg",
            AgentInputMode::PromptViaStdinClosed => "stdin",
            AgentInputMode::PromptViaTempFile => "file",
            AgentInputMode::InteractiveStdin => "interactive-stdin",
        };

        display::agent_line(&format!(
            "\x1b[2m\x1b[90m[dry-run]\x1b[0m  {} / {}  \x1b[90m(permission={permission_label}, input={input_label})\x1b[0m",
            display::agent_display(agent),
            role,
        ));
        display::agent_line(&format!("  \x1b[90mcommand :\x1b[0m {}", cmd_preview));
        display::agent_line(&format!(
            "  \x1b[90mprompt  :\x1b[0m [{prompt_char_count} chars] {prompt_path_str}",
        ));
        display::agent_blank();

        let lines: Vec<&str> = prompt.lines().collect();
        if lines.len() <= 8 {
            for line in &lines {
                display::agent_line(line);
            }
        } else {
            for line in lines.iter().take(3) {
                display::agent_line(line);
            }
            display::agent_line(&format!(
                "  \x1b[90m[{prompt_char_count} chars, {} lines - full prompt written to session]\x1b[0m",
                lines.len()
            ));
        }

        AgentOutput {
            status: 0,
            stdout: "[dry-run preview]".to_string(),
            stderr: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "windows")]
    use std::path::Path;

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_wrapper_extensions_are_detected() {
        let claude_ext = Path::new(r"C:\Users\me\AppData\Roaming\npm\claude.ps1")
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap();
        let codex_ext = Path::new(r"C:\Users\me\AppData\Roaming\npm\codex.cmd")
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap();

        assert!(claude_ext.eq_ignore_ascii_case("ps1"));
        assert!(codex_ext.eq_ignore_ascii_case("cmd"));
    }
}
