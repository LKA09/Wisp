use std::fs;
use std::path::Path;
use std::process::Command;

use crate::agent::PermissionMode;
use crate::config;
use crate::git;
use crate::language::{Language, detect, msg};
use crate::workflow::{SingleAgentArgs, SummonArgs, run_single_agent, summon as run_summon};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractiveAction {
    DryRunWorkflow {
        task: String,
    },
    ExecuteWorkflow {
        task: String,
        permission_mode: PermissionMode,
    },
    ExecuteSingleAgent {
        agent: String,
        task: String,
        permission_mode: PermissionMode,
    },
    PreviewCommands {
        query: String,
    },
    EnterPasteMode,
    Help,
    Exit,
}

pub fn parse_interactive_action(input: &str) -> InteractiveAction {
    let trimmed = input.trim();

    match trimmed {
        "" => {
            return InteractiveAction::PreviewCommands {
                query: String::new(),
            };
        }
        "/" => {
            return InteractiveAction::PreviewCommands {
                query: String::new(),
            };
        }
        "exit" | "quit" | "q" | "/exit" | "/quit" => return InteractiveAction::Exit,
        "/help" | "/commands" | "help" => return InteractiveAction::Help,
        "/paste" => return InteractiveAction::EnterPasteMode,
        _ => {}
    }

    // Multi-line input: detect trailing command on the last non-empty line.
    if trimmed.contains('\n') {
        return parse_multiline_action(trimmed);
    }

    if let Some(task) = trimmed.strip_prefix("/claude ") {
        return InteractiveAction::ExecuteSingleAgent {
            agent: "claude".into(),
            task: task.trim().into(),
            permission_mode: PermissionMode::Interactive,
        };
    }

    if let Some(task) = trimmed.strip_prefix("/codex ") {
        return InteractiveAction::ExecuteSingleAgent {
            agent: "codex".into(),
            task: task.trim().into(),
            permission_mode: PermissionMode::Interactive,
        };
    }

    if let Some(task) = trimmed.strip_prefix("/run ") {
        return parse_execute_command(task.trim(), PermissionMode::Interactive);
    }

    if let Some(task) = trimmed.strip_prefix("/exec ") {
        return parse_execute_command(task.trim(), PermissionMode::Interactive);
    }

    if let Some(task) = trimmed.strip_prefix("/auto ") {
        return parse_execute_command(task.trim(), PermissionMode::Auto);
    }

    if let Some(query) = trimmed.strip_prefix('/') {
        return InteractiveAction::PreviewCommands {
            query: query.trim().to_string(),
        };
    }

    if let Some(task) = trimmed.strip_prefix("!claude ") {
        return InteractiveAction::ExecuteSingleAgent {
            agent: "claude".into(),
            task: task.trim().into(),
            permission_mode: PermissionMode::Interactive,
        };
    }

    if let Some(task) = trimmed.strip_prefix("!codex ") {
        return InteractiveAction::ExecuteSingleAgent {
            agent: "codex".into(),
            task: task.trim().into(),
            permission_mode: PermissionMode::Interactive,
        };
    }

    if let Some(task) = trimmed.strip_prefix('!') {
        return InteractiveAction::DryRunWorkflow {
            task: task.trim().into(),
        };
    }

    if let Some(task) = trimmed.strip_prefix('~') {
        return InteractiveAction::DryRunWorkflow {
            task: task.trim().into(),
        };
    }

    InteractiveAction::DryRunWorkflow {
        task: trimmed.into(),
    }
}

/// Parse multi-line input, recognising a trailing command on the last non-empty line.
///
/// Supported trailing commands: `/run`, `/exec`, `/auto`, `/claude`, `/codex`.
/// If the last non-empty line is not a recognized command, the whole text is
/// treated as a dry-run workflow.
fn parse_multiline_action(trimmed: &str) -> InteractiveAction {
    let lines: Vec<&str> = trimmed.lines().collect();

    if let Some(pos) = lines.iter().rposition(|l| !l.trim().is_empty()) {
        let last = lines[pos].trim();
        // Build the task from all lines before the trailing command.
        let task_str = lines[..pos].join("\n");
        let task = task_str.trim_end();

        match last {
            "/run" | "/exec" => {
                return parse_execute_command(task, PermissionMode::Interactive);
            }
            "/auto" => {
                return parse_execute_command(task, PermissionMode::Auto);
            }
            "/claude" => {
                return InteractiveAction::ExecuteSingleAgent {
                    agent: "claude".into(),
                    task: task.trim().to_string(),
                    permission_mode: PermissionMode::Interactive,
                };
            }
            "/codex" => {
                return InteractiveAction::ExecuteSingleAgent {
                    agent: "codex".into(),
                    task: task.trim().to_string(),
                    permission_mode: PermissionMode::Interactive,
                };
            }
            _ => {}
        }
    }

    // No recognized trailing command — dry-run with full text.
    InteractiveAction::DryRunWorkflow {
        task: trimmed.into(),
    }
}

fn parse_execute_command(task: &str, permission_mode: PermissionMode) -> InteractiveAction {
    if let Some(rest) = task.strip_prefix("claude ") {
        return InteractiveAction::ExecuteSingleAgent {
            agent: "claude".into(),
            task: rest.trim().into(),
            permission_mode,
        };
    }

    if let Some(rest) = task.strip_prefix("codex ") {
        return InteractiveAction::ExecuteSingleAgent {
            agent: "codex".into(),
            task: rest.trim().into(),
            permission_mode,
        };
    }

    InteractiveAction::ExecuteWorkflow {
        task: task.into(),
        permission_mode,
    }
}

pub fn interactive() {
    use crate::display;
    use std::sync::mpsc;
    use std::time::Duration;

    if !config::Config::exists() {
        display::no_config_hint();
        return;
    }

    display::interactive_header();

    // Resize watcher — polls terminal width every 100 ms.
    let (resize_tx, resize_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let mut last_w = display::term_width();
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let w = display::term_width();
            if w != last_w {
                last_w = w;
                if resize_tx.send(()).is_err() {
                    break;
                }
            }
        }
    });

    // Prefer raw character-by-character input (enables live completions).
    // Fall back to line-by-line reading when raw mode is unavailable.
    if let Some(raw) = crate::input::RawConsole::new() {
        interactive_raw(raw, resize_rx);
    } else {
        interactive_lines(resize_rx);
    }
}

fn dispatch(trimmed: &str) -> bool {
    use crate::display;
    match parse_interactive_action(trimmed) {
        InteractiveAction::Exit => {
            display::goodbye();
            return false;
        }
        InteractiveAction::PreviewCommands { query } => {
            display::interactive_command_preview(&query);
        }
        InteractiveAction::Help => display::interactive_help(),
        InteractiveAction::EnterPasteMode => {
            // Normally handled in the interactive loops before dispatch.
            // If we get here, just show the paste mode hint.
            display::paste_mode_enter();
        }
        InteractiveAction::DryRunWorkflow { task } => {
            run_summon_command(&task, false, false, PermissionMode::Interactive);
        }
        InteractiveAction::ExecuteWorkflow {
            task,
            permission_mode,
        } => {
            run_summon_command(&task, true, false, permission_mode);
        }
        InteractiveAction::ExecuteSingleAgent {
            agent,
            task,
            permission_mode,
        } => {
            run_single_agent_command(&agent, &task, true, false, permission_mode);
        }
    }
    true
}

/// Raw-mode interactive loop: reads one character at a time and shows live
/// command completions when the user types a `/` prefix.
fn interactive_raw(raw: crate::input::RawConsole, resize_rx: std::sync::mpsc::Receiver<()>) {
    while let Some(line) = read_raw_line(&raw, &resize_rx) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Handle /paste before dispatch so we can collect lines until /end.
        if trimmed == "/paste" {
            let (task, cmd) = run_paste_mode_raw(&raw, &resize_rx);
            if task.is_empty() && cmd.is_none() {
                continue;
            }
            let combined = match cmd.as_deref() {
                Some(c) => format!("{}\n{}", task.trim(), c),
                None => task,
            };
            let combined_trimmed = combined.trim().to_string();
            if !combined_trimmed.is_empty() && !dispatch(&combined_trimmed) {
                break;
            }
            continue;
        }

        if !dispatch(trimmed) {
            break;
        }
    }
}

/// Collect paste block lines in raw mode until the user types `/end`.
/// Returns (task_text, optional_trailing_command).
fn run_paste_mode_raw(
    raw: &crate::input::RawConsole,
    resize_rx: &std::sync::mpsc::Receiver<()>,
) -> (String, Option<String>) {
    use crate::display;
    use std::io::Write;

    display::paste_mode_enter();

    let mut lines: Vec<String> = Vec::new();
    while let Some(line) = read_raw_line(raw, resize_rx) {
        if line.trim() == "/end" {
            break;
        }
        lines.push(line);
    }

    let char_count: usize = lines
        .iter()
        .map(|l| l.len() + 1)
        .sum::<usize>()
        .saturating_sub(1);
    display::paste_mode_captured(char_count, lines.len());

    // Prompt the user for an optional command.
    display::paste_mode_command_prompt();
    std::io::stdout().flush().ok();
    let cmd_line = read_raw_line(raw, resize_rx).unwrap_or_default();
    let cmd = if cmd_line.trim().is_empty() {
        None
    } else {
        Some(cmd_line.trim().to_string())
    };

    (lines.join("\n"), cmd)
}

/// Read one logical line in raw mode, showing live completions as the user types.
fn read_raw_line(
    raw: &crate::input::RawConsole,
    resize_rx: &std::sync::mpsc::Receiver<()>,
) -> Option<String> {
    use crate::{display, input};
    use std::io::Write;
    use std::time::Duration;

    let mut buf = String::new();
    display::redraw_prompt_with_completions(&buf);

    loop {
        // Handle pending resize events.
        if resize_rx.try_recv().is_ok() {
            display::on_resize();
            display::redraw_prompt_with_completions(&buf);
        }

        match raw.try_read_key() {
            Some(input::Key::Enter) => {
                // Clear any completion box below and advance to the next line.
                println!("\x1b[J");
                std::io::stdout().flush().ok();
                return Some(buf);
            }
            Some(input::Key::Backspace) => {
                buf.pop();
                display::redraw_prompt_with_completions(&buf);
            }
            Some(input::Key::Escape) => {
                buf.clear();
                display::redraw_prompt_with_completions(&buf);
            }
            Some(input::Key::Char(c)) => {
                buf.push(c);
                display::redraw_prompt_with_completions(&buf);
            }
            None => {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// Fallback line-by-line loop used when raw mode is unavailable.
fn interactive_lines(resize_rx: std::sync::mpsc::Receiver<()>) {
    use crate::display;
    use std::io::{self, BufRead, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    loop {
        display::interactive_prompt();
        io::stdout().flush().ok();

        let first = loop {
            if resize_rx.try_recv().is_ok() {
                display::on_resize();
                display::interactive_prompt();
                io::stdout().flush().ok();
            }
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(l) => break Some(l),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };

        let Some(first_line) = first else { break };

        // Handle /paste before the regular command dispatch.
        if first_line.trim() == "/paste" {
            let (task, cmd) = run_paste_mode_lines(&rx);
            if task.is_empty() && cmd.is_none() {
                continue;
            }
            let combined = match cmd.as_deref() {
                Some(c) => format!("{}\n{}", task.trim(), c),
                None => task,
            };
            let combined_trimmed = combined.trim().to_string();
            if !combined_trimmed.is_empty() && !dispatch(&combined_trimmed) {
                break;
            }
            continue;
        }

        // For non-paste input: if first line is a slash command, use it as-is.
        // Otherwise collect additional lines that arrive within 40 ms (paste window).
        let input = if first_line.trim_start().starts_with('/') {
            first_line
        } else {
            let mut lines = vec![first_line];
            while let Ok(l) = rx.recv_timeout(Duration::from_millis(40)) {
                lines.push(l);
            }
            lines.join("\n")
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !dispatch(trimmed) {
            break;
        }
    }
}

/// Collect paste block lines in lines mode until the user types `/end`.
/// Returns (task_text, optional_trailing_command).
fn run_paste_mode_lines(rx: &std::sync::mpsc::Receiver<String>) -> (String, Option<String>) {
    use crate::display;
    use std::io::Write;
    use std::time::Duration;

    display::paste_mode_enter();

    let mut lines: Vec<String> = Vec::new();
    while let Ok(line) = rx.recv_timeout(Duration::from_secs(120)) {
        if line.trim() == "/end" {
            break;
        }
        lines.push(line);
    }

    let char_count: usize = lines
        .iter()
        .map(|l| l.len() + 1)
        .sum::<usize>()
        .saturating_sub(1);
    display::paste_mode_captured(char_count, lines.len());

    // Prompt the user for an optional command.
    display::paste_mode_command_prompt();
    std::io::stdout().flush().ok();
    let cmd_line = rx
        .recv_timeout(Duration::from_secs(120))
        .unwrap_or_default();
    let cmd = if cmd_line.trim().is_empty() {
        None
    } else {
        Some(cmd_line.trim().to_string())
    };

    (lines.join("\n"), cmd)
}

pub fn init(force: bool) {
    use crate::display;

    display::init_header();

    let wisp_toml = Path::new("wisp.toml");
    if wisp_toml.exists() && !force {
        display::init_overwrite_hint();
    } else {
        match fs::write(wisp_toml, config::default_config_toml()) {
            Ok(_) => display::init_created("wisp.toml"),
            Err(e) => display::init_error("wisp.toml", &e),
        }
    }

    let sessions_dir = Path::new(".wisp/sessions");
    if sessions_dir.exists() {
        display::init_exists(".wisp/sessions/");
    } else {
        match fs::create_dir_all(sessions_dir) {
            Ok(_) => display::init_created(".wisp/sessions/"),
            Err(e) => display::init_error(".wisp/sessions/", &e),
        }
    }

    let instructions_file = Path::new(".wisp/instructions.md");
    if !instructions_file.exists() {
        let content =
            "# Project Instructions\n\nAdd project-specific instructions for Wisp agents here.\n";
        match fs::write(instructions_file, content) {
            Ok(_) => display::init_created(".wisp/instructions.md"),
            Err(e) => display::init_error(".wisp/instructions.md", &e),
        }
    } else {
        display::init_exists(".wisp/instructions.md");
    }

    display::init_done();
}

pub fn doctor() {
    use crate::display;

    display::doctor_header();

    let git_ok = git::git_available();
    display::doctor_check(
        "git installed",
        git_ok,
        Some("install: https://git-scm.com/downloads"),
    );

    let git_repo = git::is_git_repo();
    display::doctor_check("git repository", git_repo, Some("run: git init"));

    let config_ok = config::Config::exists();
    display::doctor_check("wisp.toml exists", config_ok, Some("run: wisp init"));

    let sessions_ok = Path::new(".wisp/sessions").exists();
    display::doctor_check(
        ".wisp/sessions/ exists",
        sessions_ok,
        Some("run: wisp init"),
    );

    println!();

    let claude_ok = cmd_available("claude", "--version");
    display::doctor_check(
        "Claude CLI  [--execute-agents, workflow]",
        claude_ok,
        Some("npm install -g @anthropic-ai/claude-code  (optional for dry-run)"),
    );

    let codex_ok = cmd_available("codex", "--version");
    display::doctor_check(
        "Codex CLI   [--execute-agents, workflow]",
        codex_ok,
        Some("npm install -g @openai/codex  (optional for dry-run)"),
    );

    let env_ok = git_ok && git_repo && config_ok && sessions_ok;
    let agents_ok = claude_ok && codex_ok;
    display::doctor_summary(env_ok, agents_ok);
}

fn cmd_available(cmd: &str, arg: &str) -> bool {
    Command::new(cmd)
        .arg(arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn summon(task: &str, execute_agents: bool, allow_dirty: bool, permission: PermissionMode) {
    run_summon_command(task, execute_agents, allow_dirty, permission);
}

pub fn ask(
    agent: &str,
    task: &str,
    execute_agents: bool,
    allow_dirty: bool,
    permission: PermissionMode,
) {
    run_single_agent_command(agent, task, execute_agents, allow_dirty, permission);
}

fn run_summon_command(
    task: &str,
    execute_agents: bool,
    allow_dirty: bool,
    permission_mode: PermissionMode,
) {
    let lang = detect(task);

    if !config::Config::exists() {
        eprintln!(
            "{}",
            msg(
                &lang,
                "Error: wisp.toml not found. Run `wisp init` first.",
                "오류: wisp.toml을 찾을 수 없습니다. 먼저 `wisp init`을 실행하세요."
            )
        );
        std::process::exit(1);
    }

    let args = SummonArgs {
        task: task.to_string(),
        execute_agents,
        allow_dirty,
        permission_mode,
        lang,
    };

    if let Err(e) = run_summon(args) {
        let lang2 = detect(task);
        eprintln!(
            "{}",
            msg(&lang2, &format!("Error: {}", e), &format!("오류: {}", e))
        );
        std::process::exit(1);
    }
}

fn run_single_agent_command(
    agent: &str,
    task: &str,
    execute_agents: bool,
    allow_dirty: bool,
    permission_mode: PermissionMode,
) {
    let lang: Language = detect(task);

    if !config::Config::exists() {
        eprintln!(
            "{}",
            msg(
                &lang,
                "Error: wisp.toml not found. Run `wisp init` first.",
                "오류: wisp.toml을 찾을 수 없습니다. 먼저 `wisp init`을 실행하세요."
            )
        );
        std::process::exit(1);
    }

    let args = SingleAgentArgs {
        agent: agent.to_string(),
        task: task.to_string(),
        execute_agents,
        allow_dirty,
        permission_mode,
        lang,
    };

    if let Err(e) = run_single_agent(args) {
        let lang2 = detect(task);
        eprintln!(
            "{}",
            msg(&lang2, &format!("Error: {}", e), &format!("오류: {}", e))
        );
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{InteractiveAction, parse_interactive_action};
    use crate::agent::PermissionMode;

    #[test]
    fn parses_slash_as_command_preview() {
        assert_eq!(
            parse_interactive_action("/"),
            InteractiveAction::PreviewCommands {
                query: String::new()
            }
        );
    }

    #[test]
    fn parses_slash_claude_direct_command() {
        assert_eq!(
            parse_interactive_action("/claude fix bug"),
            InteractiveAction::ExecuteSingleAgent {
                agent: "claude".into(),
                task: "fix bug".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn parses_claude_direct_command() {
        assert_eq!(
            parse_interactive_action("!claude fix bug"),
            InteractiveAction::ExecuteSingleAgent {
                agent: "claude".into(),
                task: "fix bug".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn parses_run_codex_command() {
        assert_eq!(
            parse_interactive_action("/run codex refactor auth"),
            InteractiveAction::ExecuteSingleAgent {
                agent: "codex".into(),
                task: "refactor auth".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn parses_bare_task_as_dry_run() {
        assert_eq!(
            parse_interactive_action("explain this repo"),
            InteractiveAction::DryRunWorkflow {
                task: "explain this repo".into()
            }
        );
    }

    #[test]
    fn parses_multiline_pasted_task_as_one_dry_run() {
        assert_eq!(
            parse_interactive_action("review this repo\nfocus on auth\nand tests"),
            InteractiveAction::DryRunWorkflow {
                task: "review this repo\nfocus on auth\nand tests".into()
            }
        );
    }

    #[test]
    fn unknown_slash_command_shows_preview() {
        assert_eq!(
            parse_interactive_action("/co"),
            InteractiveAction::PreviewCommands { query: "co".into() }
        );
    }

    // ── Multi-line paste with trailing commands ────────────────────────────────

    #[test]
    fn multiline_with_trailing_run_executes_workflow() {
        let input = "fix the auth bug\naddress the review comments\n/run";
        assert_eq!(
            parse_interactive_action(input),
            InteractiveAction::ExecuteWorkflow {
                task: "fix the auth bug\naddress the review comments".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn multiline_with_trailing_auto_executes_auto_workflow() {
        let input = "fix the auth bug\n/auto";
        assert_eq!(
            parse_interactive_action(input),
            InteractiveAction::ExecuteWorkflow {
                task: "fix the auth bug".into(),
                permission_mode: PermissionMode::Auto,
            }
        );
    }

    #[test]
    fn multiline_with_trailing_claude_runs_single_agent() {
        let input = "fix the auth bug\nfocus on tests\n/claude";
        assert_eq!(
            parse_interactive_action(input),
            InteractiveAction::ExecuteSingleAgent {
                agent: "claude".into(),
                task: "fix the auth bug\nfocus on tests".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn multiline_with_trailing_codex_runs_single_agent() {
        let input = "refactor auth module\n/codex";
        assert_eq!(
            parse_interactive_action(input),
            InteractiveAction::ExecuteSingleAgent {
                agent: "codex".into(),
                task: "refactor auth module".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn multiline_no_trailing_command_is_dry_run() {
        let input = "fix the auth bug\nfocus on tests\nlook at src/auth.rs";
        assert_eq!(
            parse_interactive_action(input),
            InteractiveAction::DryRunWorkflow {
                task: "fix the auth bug\nfocus on tests\nlook at src/auth.rs".into(),
            }
        );
    }

    #[test]
    fn trailing_command_not_included_in_task() {
        let input = "task line 1\ntask line 2\n/run";
        match parse_interactive_action(input) {
            InteractiveAction::ExecuteWorkflow { task, .. } => {
                assert_eq!(task, "task line 1\ntask line 2");
                assert!(!task.contains("/run"));
            }
            other => panic!("expected ExecuteWorkflow, got {:?}", other),
        }
    }

    #[test]
    fn trailing_command_not_included_in_single_agent_task() {
        let input = "refactor auth\ncheck edge cases\n/claude";
        match parse_interactive_action(input) {
            InteractiveAction::ExecuteSingleAgent { task, .. } => {
                assert_eq!(task, "refactor auth\ncheck edge cases");
                assert!(!task.contains("/claude"));
            }
            other => panic!("expected ExecuteSingleAgent, got {:?}", other),
        }
    }

    #[test]
    fn single_line_slash_commands_still_work() {
        assert_eq!(
            parse_interactive_action("/run fix the login bug"),
            InteractiveAction::ExecuteWorkflow {
                task: "fix the login bug".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
        assert_eq!(
            parse_interactive_action("/auto deploy to staging"),
            InteractiveAction::ExecuteWorkflow {
                task: "deploy to staging".into(),
                permission_mode: PermissionMode::Auto,
            }
        );
        assert_eq!(
            parse_interactive_action("/claude explain this code"),
            InteractiveAction::ExecuteSingleAgent {
                agent: "claude".into(),
                task: "explain this code".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
        assert_eq!(
            parse_interactive_action("/codex refactor utils"),
            InteractiveAction::ExecuteSingleAgent {
                agent: "codex".into(),
                task: "refactor utils".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn parses_paste_command() {
        assert_eq!(
            parse_interactive_action("/paste"),
            InteractiveAction::EnterPasteMode,
        );
    }

    #[test]
    fn multiline_trailing_blank_lines_ignored_in_command_detection() {
        // Trailing blank lines after /run should still be ignored.
        let input = "fix bug\n/run\n\n";
        assert_eq!(
            parse_interactive_action(input),
            InteractiveAction::ExecuteWorkflow {
                task: "fix bug".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }
}
