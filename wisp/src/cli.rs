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
    /// Bare input or multi-line with no trailing command — respects saved mode setting.
    BareTask {
        task: String,
        permission_mode: PermissionMode,
    },
    /// Explicitly requested execution (e.g. /run, /exec, /auto).
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
    ModeAction {
        arg: Option<String>,
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
        "/mode" => return InteractiveAction::ModeAction { arg: None },
        _ => {}
    }

    if let Some(arg) = trimmed.strip_prefix("/mode ") {
        return InteractiveAction::ModeAction {
            arg: Some(arg.trim().to_string()),
        };
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

    if let Some(task) = trimmed.strip_prefix("/dry ") {
        return InteractiveAction::DryRunWorkflow {
            task: task.trim().into(),
        };
    }

    if trimmed == "/dry" {
        return InteractiveAction::DryRunWorkflow {
            task: String::new(),
        };
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

    InteractiveAction::BareTask {
        task: trimmed.into(),
        permission_mode: PermissionMode::Interactive,
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
            "/dry" => {
                return InteractiveAction::DryRunWorkflow {
                    task: task.trim().to_string(),
                };
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

    // No recognized trailing command — use bare-task (respects mode setting).
    InteractiveAction::BareTask {
        task: trimmed.into(),
        permission_mode: PermissionMode::Interactive,
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
    if let Err(e) = crate::tui::run() {
        eprintln!("TUI error: {e}");
        std::process::exit(1);
    }
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

pub fn update() {
    if let Err(e) = crate::update::run() {
        eprintln!("Update failed: {e}");
        std::process::exit(1);
    }
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
    // On Windows, npm CLIs are .cmd wrappers — invoke via cmd.exe so they're found.
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", cmd, arg])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new(cmd)
            .arg(arg)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
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

pub fn mode(arg: Option<&str>) {
    use crate::{display, settings};

    let mut s = settings::Settings::load();
    match arg {
        None => {
            display::mode_status(s.execute_agents);
        }
        Some("dry" | "dry-run") => {
            s.execute_agents = false;
            if let Err(e) = s.save() {
                eprintln!("Error saving settings: {e}");
                return;
            }
            display::mode_set(false);
        }
        Some("execute" | "run" | "exec") => {
            s.execute_agents = true;
            if let Err(e) = s.save() {
                eprintln!("Error saving settings: {e}");
                return;
            }
            display::mode_set(true);
        }
        Some(other) => {
            eprintln!(
                "Unknown mode: '{other}'. Use 'dry-run' (preview only) or 'execute' (invoke agents)."
            );
        }
    }
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
    fn parses_bare_task_as_bare_task() {
        assert_eq!(
            parse_interactive_action("explain this repo"),
            InteractiveAction::BareTask {
                task: "explain this repo".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn parses_multiline_pasted_task_as_bare_task() {
        assert_eq!(
            parse_interactive_action("review this repo\nfocus on auth\nand tests"),
            InteractiveAction::BareTask {
                task: "review this repo\nfocus on auth\nand tests".into(),
                permission_mode: PermissionMode::Interactive,
            }
        );
    }

    #[test]
    fn parses_bang_prefix_as_dry_run() {
        assert_eq!(
            parse_interactive_action("!explain this repo"),
            InteractiveAction::DryRunWorkflow {
                task: "explain this repo".into()
            }
        );
    }

    #[test]
    fn parses_dry_command_as_dry_run() {
        assert_eq!(
            parse_interactive_action("/dry fix the auth bug"),
            InteractiveAction::DryRunWorkflow {
                task: "fix the auth bug".into()
            }
        );
    }

    #[test]
    fn multiline_with_trailing_dry_is_dry_run() {
        let input = "fix the auth bug\ncheck edge cases\n/dry";
        assert_eq!(
            parse_interactive_action(input),
            InteractiveAction::DryRunWorkflow {
                task: "fix the auth bug\ncheck edge cases".into(),
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
    fn multiline_no_trailing_command_is_bare_task() {
        let input = "fix the auth bug\nfocus on tests\nlook at src/auth.rs";
        assert_eq!(
            parse_interactive_action(input),
            InteractiveAction::BareTask {
                task: "fix the auth bug\nfocus on tests\nlook at src/auth.rs".into(),
                permission_mode: PermissionMode::Interactive,
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

    #[test]
    fn parses_mode_command_no_arg() {
        assert_eq!(
            parse_interactive_action("/mode"),
            InteractiveAction::ModeAction { arg: None }
        );
    }

    #[test]
    fn parses_mode_command_dry_run() {
        assert_eq!(
            parse_interactive_action("/mode dry-run"),
            InteractiveAction::ModeAction {
                arg: Some("dry-run".into())
            }
        );
    }

    #[test]
    fn parses_mode_command_execute() {
        assert_eq!(
            parse_interactive_action("/mode execute"),
            InteractiveAction::ModeAction {
                arg: Some("execute".into())
            }
        );
    }
}
