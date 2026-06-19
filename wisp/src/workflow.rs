use anyhow::Result;

use crate::agent::{AgentConfig as RunnerConfig, AgentRunner, DryRunRunner, SubprocessRunner};
use crate::config::Config;
use crate::git;
use crate::instructions::{load_instructions, LoadedInstructions};
use crate::language::{msg, Language};
use crate::session::Session;

pub struct SummonArgs {
    pub task: String,
    pub execute_agents: bool,
    pub allow_dirty: bool,
    pub lang: Language,
}

pub fn summon(args: SummonArgs) -> Result<()> {
    let lang = &args.lang;

    // 1. Load config
    let config = Config::load()?;

    println!("{}", msg(lang, "Loading project instructions...", "프로젝트 지시사항을 불러오는 중..."));

    // 2. Load project instruction files
    let instructions = load_instructions(&config);
    if !instructions.files.is_empty() {
        println!(
            "{}",
            msg(
                lang,
                &format!("  Loaded {} instruction file(s) ({} bytes)", instructions.files.len(), instructions.total_bytes),
                &format!("  지시사항 파일 {}개 로드됨 ({} 바이트)", instructions.files.len(), instructions.total_bytes)
            )
        );
    } else {
        println!("{}", msg(lang, "  No instruction files found.", "  지시사항 파일 없음."));
    }

    // 3. Check git working tree
    if !args.allow_dirty {
        if let Ok(false) = git::working_tree_clean() {
            eprintln!(
                "{}",
                msg(
                    lang,
                    "Warning: Working tree has uncommitted changes. Use --allow-dirty to proceed anyway.",
                    "경고: 커밋되지 않은 변경사항이 있습니다. --allow-dirty 플래그를 사용하면 계속할 수 있습니다."
                )
            );
        }
    }

    // 4. Create session directory
    println!("{}", msg(lang, "Creating session...", "세션을 생성하는 중..."));
    let session = Session::create()?;

    // 5. Build prompts
    let normalized_task_en = normalize_task_en(&args.task, lang);
    let instructions_text = instructions.combined();

    let implement_prompt = build_implement_prompt(&normalized_task_en, &args.task, &instructions_text);
    let patch_prompt = build_patch_prompt(&normalized_task_en, &args.task, &instructions_text);
    let review_prompt = build_review_prompt(&normalized_task_en, &args.task);
    let ship_prompt = build_ship_prompt(&normalized_task_en, &args.task);

    // 6. Save session files
    session.write("task.original.txt", &args.task)?;
    session.write(
        "task.normalized.en.md",
        &format!("# Normalized Task (English)\n\n{}\n", normalized_task_en),
    )?;
    session.write(
        "instructions.loaded.md",
        &format!("# Loaded Project Instructions\n\n{}", instructions_text),
    )?;
    session.write("prompts/claude.implement.en.md", &implement_prompt)?;
    session.write("prompts/codex.patch.en.md", &patch_prompt)?;
    session.write("prompts/claude.review.en.md", &review_prompt)?;
    session.write("prompts/codex.ship.en.md", &ship_prompt)?;

    // 7. Save git info
    match git::status() {
        Ok(s) => session.write("git/status.txt", &s)?,
        Err(_) => session.write("git/status.txt", "(git status unavailable)")?,
    }
    match git::diff() {
        Ok(d) if !d.is_empty() => session.write("git/diff.patch", &d)?,
        _ => session.write("git/diff.patch", "(no diff)")?,
    }

    // 8. Run agents or write dry-run placeholders
    if args.execute_agents {
        println!("{}", msg(lang, "Running agents...", "에이전트를 실행하는 중..."));
        run_agents(&config, &session, lang, &implement_prompt, &patch_prompt, &review_prompt, &ship_prompt, false)?;
    } else {
        println!("{}", msg(lang, "Dry-run mode. Writing placeholder outputs.", "드라이런 모드. 플레이스홀더 출력을 작성합니다."));
        run_agents(&config, &session, lang, &implement_prompt, &patch_prompt, &review_prompt, &ship_prompt, true)?;
    }

    // 9. Write summary
    let branch = git::current_branch()
        .ok()
        .flatten()
        .unwrap_or_else(|| "unknown".to_string());
    let mode = if args.execute_agents { "execute" } else { "dry-run" };
    let summary = build_summary(&args.task, &normalized_task_en, &branch, mode, &session, &instructions);
    session.write("summary.md", &summary)?;

    // 10. Final output
    println!();
    println!(
        "{}",
        msg(
            lang,
            &format!("Session saved to: {}", session.path().display()),
            &format!("세션이 저장되었습니다: {}", session.path().display())
        )
    );
    if !args.execute_agents {
        println!(
            "{}",
            msg(
                lang,
                "Tip: Pass --execute-agents to invoke real Claude and Codex CLIs.",
                "팁: --execute-agents 플래그를 사용하면 실제 Claude와 Codex CLI를 실행합니다."
            )
        );
    }
    println!("{}", msg(lang, "Done.", "완료."));

    Ok(())
}

// ---------------------------------------------------------------------------
// Task normalization
// ---------------------------------------------------------------------------

fn normalize_task_en(task: &str, lang: &Language) -> String {
    match lang {
        Language::English => task.to_string(),
        Language::Korean => {
            // MVP placeholder — production would use a translation API or LLM.
            format!(
                "[Translated from Korean] Original task: \"{}\"\n\
                 (Perform the task described in the original Korean text.)",
                task
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Prompt builders (internal — always English)
// ---------------------------------------------------------------------------

fn build_implement_prompt(normalized_task_en: &str, original_task: &str, instructions: &str) -> String {
    format!(
        r#"You are Claude acting as the implementer for Wisp.

Priority order:
1. Wisp hard safety policy
2. Current user task
3. Project instructions
4. Default agent role rules

Current user task:
{}

Original user task:
{}

Project instructions:
{}

Rules:
- Modify files only to satisfy the task.
- Keep changes minimal and maintainable.
- Do not commit.
- Do not push.
- Do not add dependencies without user approval.
- Do not modify protected files.
- If requirements are ambiguous or risky, stop and request a user decision.

Output:
- Summary
- Changed files
- Any required user decisions
"#,
        normalized_task_en, original_task, instructions
    )
}

fn build_patch_prompt(normalized_task_en: &str, original_task: &str, instructions: &str) -> String {
    format!(
        r#"You are Codex acting as the patcher for Wisp.

Priority order:
1. Wisp hard safety policy
2. Current user task
3. Project instructions
4. Default agent role rules

Review the current diff and apply minimal fixes.

Current user task:
{}

Original user task:
{}

Project instructions:
{}

Rules:
- Do not change the main implementation strategy unless necessary.
- Do not commit.
- Do not push.
- Do not add dependencies without user approval.
- If Claude and Codex disagree or the next action is unclear, request a user decision.

Output:
- Patch summary
- Changed files
- Any required user decisions
"#,
        normalized_task_en, original_task, instructions
    )
}

fn build_review_prompt(normalized_task_en: &str, original_task: &str) -> String {
    format!(
        r#"You are Claude acting as the reviewer for Wisp.

Review the current diff against the user task.

Current user task:
{}

Original user task:
{}

Rules:
- Do not edit files.
- Return one of:
  - APPROVED
  - CHANGES_REQUESTED
  - NEEDS_USER_DECISION
- Mention only blocking issues.
- If push, dependency changes, protected files, deleted files, or risky commands are involved, require user approval.
"#,
        normalized_task_en, original_task
    )
}

fn build_ship_prompt(normalized_task_en: &str, original_task: &str) -> String {
    format!(
        r#"You are Codex acting as the shipper for Wisp.

Prepare the final summary and suggest a commit message.

Current user task:
{}

Original user task:
{}

Rules:
- Do not commit.
- Do not push.
- The orchestrator must ask the user before commit or push.
- Keep the commit message concise and conventional.

Output:
- Final summary
- Suggested commit message
- Push readiness checklist
"#,
        normalized_task_en, original_task
    )
}

// ---------------------------------------------------------------------------
// Agent execution
// ---------------------------------------------------------------------------

struct AgentStep<'a> {
    agent_name: &'a str,
    role: &'a str,
    prompt: &'a str,
    output_file: &'a str,
}

fn run_agents(
    config: &Config,
    session: &Session,
    lang: &Language,
    implement_prompt: &str,
    patch_prompt: &str,
    review_prompt: &str,
    ship_prompt: &str,
    dry_run: bool,
) -> Result<()> {
    let steps = [
        AgentStep { agent_name: "claude", role: "implement", prompt: implement_prompt, output_file: "outputs/claude.implement.out.md" },
        AgentStep { agent_name: "codex",  role: "patch",     prompt: patch_prompt,     output_file: "outputs/codex.patch.out.md" },
        AgentStep { agent_name: "claude", role: "review",    prompt: review_prompt,    output_file: "outputs/claude.review.out.md" },
        AgentStep { agent_name: "codex",  role: "ship",      prompt: ship_prompt,      output_file: "outputs/codex.ship.out.md" },
    ];

    let cwd = std::env::current_dir()?;

    for step in &steps {
        let label_en = format!("  {} ({})", step.agent_name, step.role);
        let label_ko = format!("  {} ({}) 실행 중...", step.agent_name, step.role);
        println!("{}", msg(lang, &label_en, &label_ko));

        let cfg = config.agents.get(step.agent_name).cloned().unwrap_or_else(|| {
            crate::config::AgentConfig {
                cmd: step.agent_name.to_string(),
                args: vec!["-p".to_string()],
            }
        });

        let runner_config = RunnerConfig {
            name: step.agent_name.to_string(),
            cmd: cfg.cmd,
            args: cfg.args,
        };

        let output = if dry_run {
            DryRunRunner { config: runner_config }.run(step.prompt, &cwd)?
        } else {
            match (SubprocessRunner { config: runner_config }).run(step.prompt, &cwd) {
                Ok(o) => o,
                Err(e) => {
                    let msg_text = msg(
                        lang,
                        &format!("  Error running {}: {}", step.agent_name, e),
                        &format!("  {} 실행 오류: {}", step.agent_name, e),
                    );
                    eprintln!("{}", msg_text);
                    session.write(
                        step.output_file,
                        &format!("# {} - {} Error\n\nFailed to run agent: {}\n", step.agent_name, step.role, e),
                    )?;
                    continue;
                }
            }
        };

        let content = format!(
            "# {} - {} Output\n\nExit status: {}\n\n## stdout\n\n{}\n\n## stderr\n\n{}\n",
            step.agent_name, step.role, output.status, output.stdout, output.stderr
        );
        session.write(step.output_file, &content)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

fn build_summary(
    original_task: &str,
    normalized_task_en: &str,
    branch: &str,
    mode: &str,
    session: &Session,
    instructions: &LoadedInstructions,
) -> String {
    format!(
        "# Wisp Session Summary\n\n\
         ## Task\n\n\
         **Original:** {}\n\n\
         **Normalized (EN):** {}\n\n\
         ## Context\n\n\
         - Branch: {}\n\
         - Mode: {}\n\
         - Session: {}\n\
         - Instruction files loaded: {} ({} bytes{})\n\n\
         ## Agent Steps\n\n\
         1. Claude — implementer → `prompts/claude.implement.en.md`\n\
         2. Codex  — patcher    → `prompts/codex.patch.en.md`\n\
         3. Claude — reviewer   → `prompts/claude.review.en.md`\n\
         4. Codex  — shipper    → `prompts/codex.ship.en.md`\n\n\
         ## Safety Reminders\n\n\
         - Git push requires explicit user approval.\n\
         - Protected files must not be modified.\n\
         - Dangerous commands are blocked by policy.\n\
         - If agents disagree, Wisp must ask the user.\n",
        original_task,
        normalized_task_en,
        branch,
        mode,
        session.path().display(),
        instructions.files.len(),
        instructions.total_bytes,
        if instructions.truncated { ", truncated" } else { "" },
    )
}
