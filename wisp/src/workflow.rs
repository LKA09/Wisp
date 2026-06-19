use anyhow::{bail, Context, Result};
use std::time::Instant;

use crate::agent::{AgentConfig as RunnerConfig, DryRunRunner, SubprocessRunner};
use crate::config::Config;
use crate::display;
use crate::git::{self, GitSnapshot};
use crate::instructions::{load_instructions, LoadedInstructions};
use crate::language::{msg, Language};
use crate::policy;
use crate::session::Session;

pub struct SummonArgs {
    pub task: String,
    pub execute_agents: bool,
    pub allow_dirty: bool,
    pub lang: Language,
}

struct WorkflowStep<'a> {
    agent: &'a str,
    role: &'a str,
    prompt: &'a str,
    out_file: &'a str,
    meta_file: &'a str,
}

pub fn summon(args: SummonArgs) -> Result<()> {
    let lang = &args.lang;
    let config = Config::load()?;
    validate_workflow_agents(&config)?;

    if args.execute_agents && !args.allow_dirty && !git::working_tree_clean()? {
        bail!(msg(
            lang,
            "Working tree has uncommitted changes. Re-run with --allow-dirty to proceed.",
            "작업 트리에 커밋되지 않은 변경이 있습니다. 계속하려면 --allow-dirty를 명시하세요."
        ));
    }

    let branch = git::current_branch()?
        .unwrap_or_else(|| "unknown".to_string());
    if args.execute_agents && policy::is_protected_branch(&branch, &config) {
        bail!(msg(
            lang,
            &format!(
                "Refusing to execute agents on protected branch `{branch}`. Create a work branch first."
            ),
            &format!(
                "보호 브랜치 `{branch}`에서는 실제 agent 실행을 막습니다. 작업 브랜치를 만든 뒤 다시 실행하세요."
            )
        ));
    }

    let instructions = load_instructions(&config);
    let mode = if args.execute_agents { "execute" } else { "dry-run" };

    display::header(&args.task, &branch, mode, instructions.files.len());

    let session = Session::create()?;
    let initial_snapshot = git::snapshot().context("Failed to capture initial git snapshot")?;
    write_snapshot(&session, "git/before", &initial_snapshot)?;

    let normalized_task_en = normalize_task_en(&args.task, lang);
    let instructions_text = instructions.combined();
    let implement_prompt = build_implement_prompt(&normalized_task_en, &args.task, &instructions_text);
    let patch_prompt = build_patch_prompt(&normalized_task_en, &args.task, &instructions_text);
    let review_prompt = build_review_prompt(&normalized_task_en, &args.task);
    let ship_prompt = build_ship_prompt(&normalized_task_en, &args.task);

    session.write("task.original.txt", &args.task)?;
    session.write(
        "task.normalized.en.md",
        &format!("# Normalized Task (English)\n\n{}\n", normalized_task_en),
    )?;
    session.write(
        "instructions.loaded.md",
        &format!("# Loaded Project Instructions\n\n{}", instructions_text),
    )?;
    session.write("prompts/implementer.en.md", &implement_prompt)?;
    session.write("prompts/patcher.en.md", &patch_prompt)?;
    session.write("prompts/reviewer.en.md", &review_prompt)?;
    session.write("prompts/shipper.en.md", &ship_prompt)?;

    let steps = build_steps(
        &config,
        &implement_prompt,
        &patch_prompt,
        &review_prompt,
        &ship_prompt,
    );
    let cwd = std::env::current_dir()?;
    let total = steps.len();

    for (i, step) in steps.iter().enumerate() {
        if i > 0 {
            let prev_agent = steps[i - 1].agent;
            let handoff = handoff_note(prev_agent, step.agent, step.role, lang);
            display::wisp_note(&handoff);
        }

        display::agent_start(step.agent, step.role, i + 1, total);

        let cfg = config
            .agents
            .get(step.agent)
            .with_context(|| format!("Agent `{}` is not configured.", step.agent))?;

        let command_preview = format!("{} {}", cfg.cmd, cfg.args.join(" ")).trim().to_string();
        if policy::is_denied_command(&command_preview, &config) {
            bail!(format!("Configured agent command is denied by policy: {command_preview}"));
        }

        let runner_cfg = RunnerConfig {
            name: step.agent.to_string(),
            cmd: cfg.cmd.clone(),
            args: cfg.args.clone(),
        };

        let before_step = git::snapshot().context("Failed to capture pre-step git snapshot")?;
        let started = Instant::now();

        let (ok, output) = if args.execute_agents {
            run_agent_with_streaming(step, &runner_cfg, &cwd)
        } else {
            let runner = DryRunRunner { config: runner_cfg };
            (true, runner.display_and_capture(step.prompt))
        };

        let duration_ms = started.elapsed().as_millis();
        let after_step = git::snapshot().context("Failed to capture post-step git snapshot")?;
        let delta_entries = git::delta_status_entries(&before_step, &after_step);
        let violations = if args.execute_agents {
            policy::evaluate_snapshot_delta(&before_step, &after_step, &delta_entries, &config)
        } else {
            Vec::new()
        };

        display::agent_end(step.agent, ok && violations.is_empty());

        let content = format!(
            "# {} → {} Output\n\nExit status: {}\n\n## stdout\n\n{}\n\n## stderr\n\n{}\n",
            display::agent_display(step.agent),
            step.role,
            output.status,
            output.stdout,
            output.stderr
        );
        session.write(step.out_file, &content)?;
        session.write(
            step.meta_file,
            &format_step_meta(
                step,
                &command_preview,
                output.status,
                duration_ms,
                &before_step,
                &after_step,
                &delta_entries,
                &violations,
            ),
        )?;

        if !violations.is_empty() {
            bail!(format_policy_violation_error(step, &violations, lang));
        }
    }

    let final_snapshot = git::snapshot().context("Failed to capture final git snapshot")?;
    write_snapshot(&session, "git/after", &final_snapshot)?;

    let summary = build_summary(
        &args.task,
        &normalized_task_en,
        &branch,
        mode,
        &session,
        &instructions,
        &config,
    );
    session.write("summary.md", &summary)?;

    display::finish(&session.path().display().to_string(), !args.execute_agents);
    Ok(())
}

fn validate_workflow_agents(config: &Config) -> Result<()> {
    let roles = [
        ("implementer", config.workflow.implementer.as_str()),
        ("patcher", config.workflow.patcher.as_str()),
        ("reviewer", config.workflow.reviewer.as_str()),
        ("shipper", config.workflow.shipper.as_str()),
    ];

    for (role, agent) in roles {
        if !config.agents.contains_key(agent) {
            bail!("Workflow role `{role}` references unknown agent `{agent}`.");
        }
    }

    Ok(())
}

fn build_steps<'a>(
    config: &'a Config,
    implement_prompt: &'a str,
    patch_prompt: &'a str,
    review_prompt: &'a str,
    ship_prompt: &'a str,
) -> Vec<WorkflowStep<'a>> {
    vec![
        WorkflowStep {
            agent: config.workflow.implementer.as_str(),
            role: "implement",
            prompt: implement_prompt,
            out_file: "outputs/implement.out.md",
            meta_file: "outputs/implement.meta.txt",
        },
        WorkflowStep {
            agent: config.workflow.patcher.as_str(),
            role: "patch",
            prompt: patch_prompt,
            out_file: "outputs/patch.out.md",
            meta_file: "outputs/patch.meta.txt",
        },
        WorkflowStep {
            agent: config.workflow.reviewer.as_str(),
            role: "review",
            prompt: review_prompt,
            out_file: "outputs/review.out.md",
            meta_file: "outputs/review.meta.txt",
        },
        WorkflowStep {
            agent: config.workflow.shipper.as_str(),
            role: "ship",
            prompt: ship_prompt,
            out_file: "outputs/ship.out.md",
            meta_file: "outputs/ship.meta.txt",
        },
    ]
}

fn run_agent_with_streaming(
    step: &WorkflowStep<'_>,
    runner_cfg: &RunnerConfig,
    cwd: &std::path::Path,
) -> (bool, crate::agent::AgentOutput) {
    let runner = SubprocessRunner {
        config: runner_cfg.clone(),
    };
    let mut spinner = display::ThinkingSpinner::start();
    let mut at_line_start = true;
    let mut first_chunk = true;

    let result = runner.run_streaming(step.prompt, cwd, |chunk| {
        use std::io::Write;
        if first_chunk {
            spinner.stop();
            first_chunk = false;
        }

        for ch in chunk.chars() {
            if at_line_start {
                print!("  │  ");
                at_line_start = false;
            }
            match ch {
                '\n' => {
                    println!();
                    at_line_start = true;
                }
                '\r' => {}
                c => print!("{c}"),
            }
        }
        let _ = std::io::stdout().flush();
    });

    spinner.stop();
    if !at_line_start {
        println!();
    }

    match result {
        Ok(out) => {
            if !out.stderr.is_empty() {
                display::agent_blank();
                for line in out.stderr.lines() {
                    display::agent_line(&format!("\x1b[90m[stderr] {line}\x1b[0m"));
                }
            }
            (out.status == 0, out)
        }
        Err(e) => {
            display::agent_line(&format!("\x1b[91merror: {e}\x1b[0m"));
            (
                false,
                crate::agent::AgentOutput {
                    status: -1,
                    stdout: String::new(),
                    stderr: e.to_string(),
                },
            )
        }
    }
}

fn write_snapshot(session: &Session, prefix: &str, snapshot: &GitSnapshot) -> Result<()> {
    session.write(
        &format!("{prefix}/head.txt"),
        &snapshot.head.clone().unwrap_or_else(|| "unknown".to_string()),
    )?;
    session.write(
        &format!("{prefix}/branch.txt"),
        &snapshot.branch.clone().unwrap_or_else(|| "unknown".to_string()),
    )?;
    session.write(&format!("{prefix}/status.porcelain.txt"), &snapshot.status_raw)?;
    session.write(
        &format!("{prefix}/diff.name-status.txt"),
        &snapshot.diff_name_status,
    )?;
    Ok(())
}

fn format_step_meta(
    step: &WorkflowStep<'_>,
    command_preview: &str,
    exit_code: i32,
    duration_ms: u128,
    before: &GitSnapshot,
    after: &GitSnapshot,
    delta_entries: &[git::StatusEntry],
    violations: &[policy::PolicyViolation],
) -> String {
    let changed_files = if delta_entries.is_empty() {
        "(none)".to_string()
    } else {
        delta_entries
            .iter()
            .map(|entry| {
                format!(
                    "{}{} {}",
                    entry.index_status, entry.worktree_status, entry.path
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let violations = if violations.is_empty() {
        "(none)".to_string()
    } else {
        violations
            .iter()
            .map(|violation| violation.message.clone())
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "role={}\nagent={}\ncommand={}\nexit_code={}\nduration_ms={}\nbranch_before={}\nbranch_after={}\nhead_before={}\nhead_after={}\nchanged_files=\n{}\n\npolicy_violations=\n{}\n",
        step.role,
        step.agent,
        command_preview,
        exit_code,
        duration_ms,
        before.branch.as_deref().unwrap_or("unknown"),
        after.branch.as_deref().unwrap_or("unknown"),
        before.head.as_deref().unwrap_or("unknown"),
        after.head.as_deref().unwrap_or("unknown"),
        changed_files,
        violations
    )
}

fn format_policy_violation_error(
    step: &WorkflowStep<'_>,
    violations: &[policy::PolicyViolation],
    lang: &Language,
) -> String {
    let details = violations
        .iter()
        .map(|violation| violation.message.clone())
        .collect::<Vec<_>>()
        .join("; ");

    msg(
        lang,
        &format!(
            "Policy blocked {} ({}) after execution: {}",
            display::agent_display(step.agent),
            step.role,
            details
        ),
        &format!(
            "정책 위반으로 {} ({}) 단계 실행 후 중단했습니다: {}",
            display::agent_display(step.agent),
            step.role,
            details
        ),
    )
}

fn handoff_note(from: &str, to: &str, role: &str, lang: &Language) -> String {
    let from_name = display::agent_display(from);
    let to_name = display::agent_display(to);
    match lang {
        Language::Korean => format!("{from_name} 완료. {to_name} ({role}) 단계로 넘깁니다."),
        Language::English => format!("{from_name} done. Handing off to {to_name} ({role})."),
    }
}

fn normalize_task_en(task: &str, lang: &Language) -> String {
    match lang {
        Language::English => task.to_string(),
        Language::Korean => format!(
            "[Translated from Korean] Original task: \"{}\"\n(Perform the task described in the original Korean text.)",
            task
        ),
    }
}

fn build_implement_prompt(normalized_task_en: &str, original_task: &str, instructions: &str) -> String {
    format!(
        r#"You are the implementer for Wisp.

Priority order:
1. Wisp runtime safety policy
2. Current user task
3. Project instructions
4. Default role behavior

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
        r#"You are the patcher for Wisp.

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
- If the next action is unclear, request a user decision.

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
        r#"You are the reviewer for Wisp.

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
        r#"You are the shipper for Wisp.

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

fn build_summary(
    original_task: &str,
    normalized_task_en: &str,
    branch: &str,
    mode: &str,
    session: &Session,
    instructions: &LoadedInstructions,
    config: &Config,
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
         ## Workflow\n\n\
         1. {} → implement\n\
         2. {} → patch\n\
         3. {} → review\n\
         4. {} → ship\n\n\
         ## Runtime Safety\n\n\
         - Interactive mode defaults to dry-run.\n\
         - Dirty working tree blocks execution unless --allow-dirty is set.\n\
         - Protected branches block --execute-agents.\n\
         - Post-step git snapshots are audited for commits, protected files, dependency files, and deletions.\n",
        original_task,
        normalized_task_en,
        branch,
        mode,
        session.path().display(),
        instructions.files.len(),
        instructions.total_bytes,
        if instructions.truncated { ", truncated" } else { "" },
        config.workflow.implementer,
        config.workflow.patcher,
        config.workflow.reviewer,
        config.workflow.shipper,
    )
}
