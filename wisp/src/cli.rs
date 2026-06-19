use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config;
use crate::git;
use crate::language::{detect, msg};
use crate::workflow::{summon as run_summon, SummonArgs};

pub fn interactive() {
    use crate::display;
    use std::io::{self, Write};

    if !config::Config::exists() {
        display::no_config_hint();
        return;
    }

    display::interactive_header();

    loop {
        display::interactive_prompt();
        io::stdout().flush().ok();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break,
            Err(_) => break,
            Ok(_) => {}
        }

        let task = input.trim();
        if task.is_empty() {
            continue;
        }

        match task {
            "exit" | "quit" | "q" | "/exit" | "/quit" => {
                display::goodbye();
                break;
            }
            "/help" | "help" => {
                display::interactive_help();
                continue;
            }
            _ => {}
        }

        let (task_str, execute) = if let Some(t) = task.strip_prefix('!') {
            (t.trim(), true)
        } else if let Some(t) = task.strip_prefix('~') {
            (t.trim(), false)
        } else {
            (task, false)
        };

        if task_str.is_empty() {
            continue;
        }

        let lang = detect(task_str);
        let args = SummonArgs {
            task: task_str.to_string(),
            execute_agents: execute,
            allow_dirty: false,
            lang,
        };

        if let Err(e) = run_summon(args) {
            let lang2 = detect(task_str);
            eprintln!(
                "{}",
                msg(&lang2, &format!("Error: {}", e), &format!("오류: {}", e))
            );
        }
    }
}

pub fn print_intro() {
    println!("Wisp\n");
    println!("A local personal coding agent.\n");
    println!("Usage:");
    println!("  wisp init");
    println!("  wisp doctor");
    println!("  wisp summon \"<task>\"");
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
        let content = "# Project Instructions\n\nAdd project-specific instructions for Wisp agents here.\n";
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
    check("Claude CLI - implementer + reviewer  [--execute-agents]", claude_ok);
    if !claude_ok {
        println!("    npm install -g @anthropic-ai/claude-code");
        println!("    (not needed for dry-run mode)");
    }

    let codex_ok = cmd_available("codex", "--version");
    check("Codex CLI  - patcher + shipper       [--execute-agents]", codex_ok);
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

pub fn summon(task: &str, execute_agents: bool, allow_dirty: bool) {
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
