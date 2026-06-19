use anyhow::Result;

use crate::agent::{AgentConfig as RunnerConfig, DryRunRunner, SubprocessRunner};
use crate::config::Config;
use crate::display;
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

    // 2. Load project instruction files
    let instructions = load_instructions(&config);

    // 3. Check git working tree
    if !args.allow_dirty {
        if let Ok(false) = git::working_tree_clean() {
            eprintln!(
                "{}",
                msg(
                    lang,
                    "Warning: working tree has uncommitted changes. Use --allow-dirty to proceed.",
                    "경고: 커밋되지 않은 변경사항이 있습니다. --allow-dirty 플래그를 사용하면 계속할 수 있습니다."
                )
            );
        }
    }

    let branch = git::current_branch()
        .ok()
        .flatten()
        .unwrap_or_else(|| "unknown".to_string());
    let mode = if args.execute_agents { "execute" } else { "dry-run" };

    // ── Conversation header ──────────────────────────────────────────────────
    display::header(&args.task, &branch, mode, instructions.files.len());

    // 4. Create session
    let session = Session::create()?;

    // 5. Build prompts
    let normalized_task_en = normalize_task_en(&args.task, lang);
    let instructions_text = instructions.combined();

    let implement_prompt = build_implement_prompt(&normalized_task_en, &args.task, &instructions_text);
    let patch_prompt     = build_patch_prompt(&normalized_task_en, &args.task, &instructions_text);
    let review_prompt    = build_review_prompt(&normalized_task_en, &args.task);
    let ship_prompt      = build_ship_prompt(&normalized_task_en, &args.task);

    // 6. Save session files (silent — conversation UI is the focus)
    session.write("task.original.txt", &args.task)?;
    session.write("task.normalized.en.md", &format!("# Normalized Task (English)\n\n{}\n", normalized_task_en))?;
    session.write("instructions.loaded.md", &format!("# Loaded Project Instructions\n\n{}", instructions_text))?;
    session.write("prompts/claude.implement.en.md", &implement_prompt)?;
    session.write("prompts/codex.patch.en.md", &patch_prompt)?;
    session.write("prompts/claude.review.en.md", &review_prompt)?;
    session.write("prompts/codex.ship.en.md", &ship_prompt)?;

    match git::status() {
        Ok(s) => session.write("git/status.txt", &s)?,
        Err(_) => session.write("git/status.txt", "(git status unavailable)")?,
    }
    match git::diff() {
        Ok(d) if !d.is_empty() => session.write("git/diff.patch", &d)?,
        _ => session.write("git/diff.patch", "(no diff)")?,
    }

    // ── Agent conversation ───────────────────────────────────────────────────
    let steps: &[(&str, &str, &str, &str)] = &[
        ("claude", "implement", &implement_prompt, "outputs/claude.implement.out.md"),
        ("codex",  "patch",     &patch_prompt,     "outputs/codex.patch.out.md"),
        ("claude", "review",    &review_prompt,     "outputs/claude.review.out.md"),
        ("codex",  "ship",      &ship_prompt,       "outputs/codex.ship.out.md"),
    ];

    let cwd = std::env::current_dir()?;
    let total = steps.len();

    for (i, &(agent, role, prompt, out_file)) in steps.iter().enumerate() {
        let step = i + 1;

        // Handoff narration between steps
        if i > 0 {
            let prev_agent = steps[i - 1].0;
            let handoff = handoff_note(prev_agent, agent, role, lang);
            display::wisp_note(&handoff);
        }

        display::agent_start(agent, role, step, total);

        let cfg = config.agents.get(agent).cloned().unwrap_or_else(|| {
            crate::config::AgentConfig {
                cmd: agent.to_string(),
                args: vec!["-p".to_string()],
            }
        });

        let runner_cfg = RunnerConfig {
            name: agent.to_string(),
            cmd: cfg.cmd.clone(),
            args: cfg.args.clone(),
        };

        let (ok, output) = if args.execute_agents {
            let runner = SubprocessRunner { config: runner_cfg };
            match runner.run_streaming(prompt, &cwd, |line| display::agent_line(line)) {
                Ok(out) => {
                    if !out.stderr.is_empty() {
                        display::agent_blank();
                        for line in out.stderr.lines() {
                            display::agent_line(&format!("\x1b[90m[stderr] {}\x1b[0m", line));
                        }
                    }
                    let ok = out.status == 0;
                    (ok, out)
                }
                Err(e) => {
                    display::agent_line(&format!("\x1b[91merror: {}\x1b[0m", e));
                    let out = crate::agent::AgentOutput {
                        status: -1,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    };
                    (false, out)
                }
            }
        } else {
            let runner = DryRunRunner { config: runner_cfg };
            let out = runner.display_and_capture(prompt);
            (true, out)
        };

        display::agent_end(agent, ok);

        let content = format!(
            "# {} — {} Output\n\nExit status: {}\n\n## stdout\n\n{}\n\n## stderr\n\n{}\n",
            display::agent_display(agent), role, output.status, output.stdout, output.stderr
        );
        session.write(out_file, &content)?;
    }

    // 9. Write summary
    let summary = build_summary(&args.task, &normalized_task_en, &branch, mode, &session, &instructions);
    session.write("summary.md", &summary)?;

    // ── Footer ───────────────────────────────────────────────────────────────
    display::finish(&session.path().display().to_string(), !args.execute_agents);

    Ok(())
}

// ─── Handoff narration ────────────────────────────────────────────────────────

fn handoff_note(from: &str, to: &str, role: &str, lang: &Language) -> String {
    let from_name = display::agent_display(from);
    let to_name   = display::agent_display(to);
    match lang {
        Language::Korean => format!("{} 완료. {} → {} ({})로 전달합니다.", from_name, from_name, to_name, role),
        Language::English => format!("{} done. Handing off to {} ({}).", from_name, to_name, role),
    }
}

// ─── Task normalization ───────────────────────────────────────────────────────

fn normalize_task_en(task: &str, lang: &Language) -> String {
    match lang {
        Language::English => task.to_string(),
        Language::Korean => format!(
            "[Translated from Korean] Original task: \"{}\"\n\
             (Perform the task described in the original Korean text.)",
            task
        ),
    }
}

// ─── Prompt builders (internal — always English) ──────────────────────────────

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

// ─── Summary ──────────────────────────────────────────────────────────────────

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
