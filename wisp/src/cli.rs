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
    Help,
    Exit,
}

pub fn parse_interactive_action(input: &str) -> InteractiveAction {
    let trimmed = input.trim();

    match trimmed {
        "" => return InteractiveAction::Help,
        "exit" | "quit" | "q" | "/exit" | "/quit" => return InteractiveAction::Exit,
        "/help" | "help" => return InteractiveAction::Help,
        _ => {}
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
    use std::io::{self, BufRead, Write};
    use std::sync::mpsc;

    if !config::Config::exists() {
        display::no_config_hint();
        return;
    }

    display::interactive_header();
    let (tx, rx) = mpsc::channel::<String>();

    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
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

        let Some(input) = read_interactive_message(&rx) else {
            break;
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_interactive_action(trimmed) {
            InteractiveAction::Exit => {
                display::goodbye();
                break;
            }
            InteractiveAction::Help => display::interactive_help(),
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
    }
}

fn read_interactive_message(rx: &std::sync::mpsc::Receiver<String>) -> Option<String> {
    let first = rx.recv().ok()?;
    let mut lines = vec![first];

    while let Ok(line) = rx.recv_timeout(std::time::Duration::from_millis(40)) {
        lines.push(line);
    }

    Some(lines.join("\n"))
}

pub fn print_intro() {
    println!("Wisp\n");
    println!("A local personal coding agent.\n");
    println!("Usage:");
    println!("  wisp init");
    println!("  wisp doctor");
    println!("  wisp summon \"<task>\"");
    println!("  wisp ask <agent> \"<task>\" --execute-agents");
}

pub fn init(force: bool) {
    let wisp_toml = Path::new("wisp.toml");

    if wisp_toml.exists() && !force {
        println!("wisp.toml already exists. Use --force to overwrite.");
    } else {
        match fs::write(wisp_toml, config::default_config_toml()) {
            Ok(_) => println!("Created wisp.toml"),
            Err(e) => eprintln!("Failed to create wisp.toml: {}", e),
        }
    }

    let sessions_dir = Path::new(".wisp/sessions");
    if !sessions_dir.exists() {
        match fs::create_dir_all(sessions_dir) {
            Ok(_) => println!("Created .wisp/sessions/"),
            Err(e) => eprintln!("Failed to create .wisp/sessions/: {}", e),
        }
    } else {
        println!(".wisp/sessions/ already exists.");
    }

    let instructions_file = Path::new(".wisp/instructions.md");
    if !instructions_file.exists() {
        let content =
            "# Project Instructions\n\nAdd project-specific instructions for Wisp agents here.\n";
        match fs::write(instructions_file, content) {
            Ok(_) => println!("Created .wisp/instructions.md"),
            Err(e) => eprintln!("Failed to create .wisp/instructions.md: {}", e),
        }
    }

    println!("\nWisp initialized. Edit wisp.toml to configure agents and workflow.");
}

pub fn doctor() {
    println!("Wisp Doctor\n");

    let git_ok = git::git_available();
    check("Git installed", git_ok);
    if !git_ok {
        println!("    Install: https://git-scm.com/downloads");
    }

    let git_repo = git::is_git_repo();
    check("Git repository", git_repo);
    if !git_repo {
        println!("    Run: git init");
    }

    let claude_ok = cmd_available("claude", "--version");
    check(
        "Claude CLI - direct + workflow      [--execute-agents]",
        claude_ok,
    );
    if !claude_ok {
        println!("    npm install -g @anthropic-ai/claude-code");
        println!("    (not needed for dry-run mode)");
    }

    let codex_ok = cmd_available("codex", "--version");
    check(
        "Codex CLI  - direct + workflow      [--execute-agents]",
        codex_ok,
    );
    if !codex_ok {
        println!("    npm install -g @openai/codex");
        println!("    (not needed for dry-run mode)");
    }

    let config_ok = config::Config::exists();
    check("wisp.toml exists", config_ok);
    if !config_ok {
        println!("    Run: wisp init");
    }

    let sessions_ok = Path::new(".wisp/sessions").exists();
    check(".wisp/sessions/ exists", sessions_ok);
    if !sessions_ok {
        println!("    Run: wisp init");
    }

    println!();
    if git_ok && git_repo && config_ok && sessions_ok {
        if claude_ok && codex_ok {
            println!("All checks passed. Wisp is fully ready.");
        } else {
            println!("Wisp is ready (dry-run mode).");
            println!("Install Claude CLI and Codex CLI to enable --execute-agents.");
        }
    } else {
        println!("Some checks failed. Run `wisp init` and install missing tools.");
    }
}

fn check(label: &str, ok: bool) {
    let status = if ok { "OK  " } else { "FAIL" };
    println!("  [{}] {}", status, label);
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
                "?ㅻ쪟: wisp.toml??李얠쓣 ???놁뒿?덈떎. 癒쇱? `wisp init`???ㅽ뻾?섏꽭??"
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
            msg(&lang2, &format!("Error: {}", e), &format!("?ㅻ쪟: {}", e))
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
                "?ㅻ쪟: wisp.toml??李얠쓣 ???놁뒿?덈떎. 癒쇱? `wisp init`???ㅽ뻾?섏꽭??"
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
            msg(&lang2, &format!("Error: {}", e), &format!("?ㅻ쪟: {}", e))
        );
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{InteractiveAction, parse_interactive_action};
    use crate::agent::PermissionMode;

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
}
