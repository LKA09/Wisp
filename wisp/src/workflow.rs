use anyhow::{Context, Result, bail};
use std::time::Instant;

use crate::agent::{
    AgentOutput, AgentRunOptions, DryRunRunner, PermissionMode, SubprocessRunner, prepare_command,
    resolve_input_mode,
};
use crate::config::Config;
use crate::display;
use crate::git::{self, GitSnapshot};
use crate::instructions::{LoadedInstructions, load_instructions};
use crate::language::{Language, msg};
use crate::policy::{self, ApprovalDecision};
use crate::session::Session;

pub struct SummonArgs {
    pub task: String,
    pub execute_agents: bool,
    pub allow_dirty: bool,
    pub permission_mode: PermissionMode,
    pub lang: Language,
}

pub struct SingleAgentArgs {
    pub agent: String,
    pub task: String,
    pub execute_agents: bool,
    pub allow_dirty: bool,
    pub permission_mode: PermissionMode,
    pub lang: Language,
}

struct WorkflowStep {
    agent: String,
    role: &'static str,
    prompt: String,
    prompt_file: String,
    out_file: &'static str,
    meta_file: &'static str,
}

pub fn summon(args: SummonArgs) -> Result<()> {
    let config = Config::load()?;
    validate_workflow_agents(&config)?;
    let branch =
        validate_execute_preconditions(&config, args.execute_agents, args.allow_dirty, &args.lang)?;

    let instructions = load_instructions(&config);
    let mode = mode_label(args.execute_agents, args.permission_mode);
    display::header(&args.task, &branch, &mode, instructions.files.len());

    let session = Session::create()?;
    let initial_snapshot = git::snapshot().context("Failed to capture initial git snapshot")?;
    write_snapshot(&session, "git/before", &initial_snapshot)?;

    let normalized_task_en = normalize_task_en(&args.task, &args.lang);
    let instructions_text = instructions.combined();
    let steps = build_workflow_steps(&config, &normalized_task_en, &args.task, &instructions_text);

    session.write("task.original.txt", &args.task)?;
    session.write(
        "task.normalized.en.md",
        &format!("# Normalized Task (English)\n\n{}\n", normalized_task_en),
    )?;
    session.write(
        "instructions.loaded.md",
        &format!("# Loaded Project Instructions\n\n{}", instructions_text),
    )?;

    let cwd = std::env::current_dir()?;
    let total = steps.len();

    for (i, step) in steps.iter().enumerate() {
        session.write(&step.prompt_file, &step.prompt)?;

        if i > 0 {
            let prev_agent = steps[i - 1].agent.as_str();
            display::wisp_note(&handoff_note(
                prev_agent,
                step.agent.as_str(),
                step.role,
                &args.lang,
            ));
        }

        display::agent_start(step.agent.as_str(), step.role, i + 1, total);
        let cfg = config
            .agents
            .get(step.agent.as_str())
            .with_context(|| format!("Agent `{}` is not configured.", step.agent))?;

        let prompt_path = session.path().join(&step.prompt_file);
        let prepared = prepare_command(
            cfg,
            step.agent.as_str(),
            &args.task,
            session.path(),
            &step.prompt,
            &prompt_path,
            args.permission_mode,
        );
        let command_preview = format!("{} {}", prepared.cmd, prepared.args.join(" "));
        if policy::is_denied_command(&command_preview, &config) {
            bail!(format!(
                "Configured agent command is denied by policy: {command_preview}"
            ));
        }

        let before_step = git::snapshot().context("Failed to capture pre-step git snapshot")?;
        let started = Instant::now();
        let output = if args.execute_agents {
            run_agent_with_streaming(
                &prepared,
                resolve_input_mode(&cfg.input),
                args.permission_mode,
                &cwd,
            )
        } else {
            DryRunRunner {
                options: AgentRunOptions {
                    permission_mode: args.permission_mode,
                    input_mode: resolve_input_mode(&cfg.input),
                    capture_output: true,
                    stream_output: true,
                },
            }
            .display_and_capture(&prepared, &step.prompt)
        };

        let duration_ms = started.elapsed().as_millis();
        let after_step = git::snapshot().context("Failed to capture post-step git snapshot")?;
        let delta_entries = git::delta_status_entries(&before_step, &after_step);
        let violations = if args.execute_agents {
            policy::evaluate_snapshot_delta(&before_step, &after_step, &delta_entries, &config)
        } else {
            Vec::new()
        };

        session.write(
            step.out_file,
            &format_agent_output(step.agent.as_str(), step.role, &output),
        )?;
        session.write(
            step.meta_file,
            &format_step_meta(
                step.role,
                step.agent.as_str(),
                &command_preview,
                &prompt_path.display().to_string(),
                &output,
                duration_ms,
                &before_step,
                &after_step,
                &delta_entries,
                &violations,
            ),
        )?;

        match handle_policy_violations(
            &violations,
            &config,
            &args.lang,
            step.agent.as_str(),
            step.role,
        )? {
            true => display::agent_end(step.agent.as_str(), output.status == 0),
            false => {
                display::agent_end(step.agent.as_str(), false);
                bail!(format_policy_violation_error(
                    step.agent.as_str(),
                    step.role,
                    &violations,
                    &args.lang
                ));
            }
        }
    }

    finalize_workflow_summary(
        &session,
        &args.task,
        &normalized_task_en,
        &branch,
        &mode,
        &instructions,
        &config,
    )?;
    display::finish(&session.path().display().to_string(), !args.execute_agents);
    Ok(())
}

pub fn run_single_agent(args: SingleAgentArgs) -> Result<()> {
    let config = Config::load()?;
    let branch =
        validate_execute_preconditions(&config, args.execute_agents, args.allow_dirty, &args.lang)?;

    let agent_cfg = config
        .agents
        .get(&args.agent)
        .with_context(|| format!("Agent `{}` is not configured.", args.agent))?;
    let instructions = load_instructions(&config);
    let mode = mode_label(args.execute_agents, args.permission_mode);
    display::header(
        &args.task,
        &branch,
        &format!("single-agent {mode}"),
        instructions.files.len(),
    );

    let session = Session::create()?;
    let initial_snapshot = git::snapshot().context("Failed to capture initial git snapshot")?;
    write_snapshot(&session, "git/before", &initial_snapshot)?;

    let normalized_task_en = normalize_task_en(&args.task, &args.lang);
    let instructions_text = instructions.combined();
    let prompt = build_direct_agent_prompt(
        &args.agent,
        &normalized_task_en,
        &args.task,
        &instructions_text,
    );
    let prompt_file = format!("prompts/{}.md", args.agent);
    session.write("task.original.txt", &args.task)?;
    session.write(
        "task.normalized.en.md",
        &format!("# Normalized Task (English)\n\n{}\n", normalized_task_en),
    )?;
    session.write(
        "instructions.loaded.md",
        &format!("# Loaded Project Instructions\n\n{}", instructions_text),
    )?;
    session.write(&prompt_file, &prompt)?;

    let prompt_path = session.path().join(&prompt_file);
    let prepared = prepare_command(
        agent_cfg,
        &args.agent,
        &args.task,
        session.path(),
        &prompt,
        &prompt_path,
        args.permission_mode,
    );
    let command_preview = format!("{} {}", prepared.cmd, prepared.args.join(" "));
    if policy::is_denied_command(&command_preview, &config) {
        bail!(format!(
            "Configured agent command is denied by policy: {command_preview}"
        ));
    }

    display::agent_start(&args.agent, "direct", 1, 1);
    let cwd = std::env::current_dir()?;
    let before_step = git::snapshot().context("Failed to capture pre-step git snapshot")?;
    let started = Instant::now();
    let output = if args.execute_agents {
        run_agent_with_streaming(
            &prepared,
            resolve_input_mode(&agent_cfg.input),
            args.permission_mode,
            &cwd,
        )
    } else {
        DryRunRunner {
            options: AgentRunOptions {
                permission_mode: args.permission_mode,
                input_mode: resolve_input_mode(&agent_cfg.input),
                capture_output: true,
                stream_output: true,
            },
        }
        .display_and_capture(&prepared, &prompt)
    };
    let duration_ms = started.elapsed().as_millis();
    let after_step = git::snapshot().context("Failed to capture post-step git snapshot")?;
    let delta_entries = git::delta_status_entries(&before_step, &after_step);
    let violations = if args.execute_agents {
        policy::evaluate_snapshot_delta(&before_step, &after_step, &delta_entries, &config)
    } else {
        Vec::new()
    };

    session.write(
        "outputs/direct.out.md",
        &format_agent_output(&args.agent, "direct", &output),
    )?;
    session.write(
        "outputs/direct.meta.txt",
        &format_step_meta(
            "direct",
            &args.agent,
            &command_preview,
            &prompt_path.display().to_string(),
            &output,
            duration_ms,
            &before_step,
            &after_step,
            &delta_entries,
            &violations,
        ),
    )?;

    match handle_policy_violations(&violations, &config, &args.lang, &args.agent, "direct")? {
        true => display::agent_end(&args.agent, output.status == 0),
        false => {
            display::agent_end(&args.agent, false);
            bail!(format_policy_violation_error(
                &args.agent,
                "direct",
                &violations,
                &args.lang,
            ));
        }
    }

    finalize_single_agent_summary(
        &session,
        &args.task,
        &normalized_task_en,
        &branch,
        &mode,
        &instructions,
        &args.agent,
    )?;
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

fn validate_execute_preconditions(
    config: &Config,
    execute_agents: bool,
    allow_dirty: bool,
    lang: &Language,
) -> Result<String> {
    if execute_agents && !allow_dirty && !git::working_tree_clean()? {
        bail!(msg(
            lang,
            "Working tree has uncommitted changes. Re-run with --allow-dirty to proceed.",
            "?묒뾽 ?몃━??而ㅻ컠?섏? ?딆? 蹂寃쎌씠 ?덉뒿?덈떎. 怨꾩냽?섎젮硫?--allow-dirty瑜?紐낆떆?섏꽭??"
        ));
    }

    let branch = git::current_branch()?.unwrap_or_else(|| "unknown".to_string());
    if execute_agents && policy::is_protected_branch(&branch, config) {
        bail!(msg(
            lang,
            &format!(
                "Refusing to execute agents on protected branch `{branch}`. Create a work branch first."
            ),
            &format!(
                "蹂댄샇 釉뚮옖移?`{branch}`?먯꽌???ㅼ젣 agent ?ㅽ뻾??留됱뒿?덈떎. ?묒뾽 釉뚮옖移섎? 留뚮뱺 ???ㅼ떆 ?ㅽ뻾?섏꽭??"
            )
        ));
    }

    Ok(branch)
}

fn build_workflow_steps(
    config: &Config,
    normalized_task_en: &str,
    original_task: &str,
    instructions: &str,
) -> Vec<WorkflowStep> {
    vec![
        WorkflowStep {
            agent: config.workflow.implementer.clone(),
            role: "implement",
            prompt: build_implement_prompt(normalized_task_en, original_task, instructions),
            prompt_file: "prompts/implementer.en.md".into(),
            out_file: "outputs/implement.out.md",
            meta_file: "outputs/implement.meta.txt",
        },
        WorkflowStep {
            agent: config.workflow.patcher.clone(),
            role: "patch",
            prompt: build_patch_prompt(normalized_task_en, original_task, instructions),
            prompt_file: "prompts/patcher.en.md".into(),
            out_file: "outputs/patch.out.md",
            meta_file: "outputs/patch.meta.txt",
        },
        WorkflowStep {
            agent: config.workflow.reviewer.clone(),
            role: "review",
            prompt: build_review_prompt(normalized_task_en, original_task),
            prompt_file: "prompts/reviewer.en.md".into(),
            out_file: "outputs/review.out.md",
            meta_file: "outputs/review.meta.txt",
        },
        WorkflowStep {
            agent: config.workflow.shipper.clone(),
            role: "ship",
            prompt: build_ship_prompt(normalized_task_en, original_task),
            prompt_file: "prompts/shipper.en.md".into(),
            out_file: "outputs/ship.out.md",
            meta_file: "outputs/ship.meta.txt",
        },
    ]
}

fn run_agent_with_streaming(
    prepared: &crate::agent::PreparedAgentCommand,
    input_mode: crate::agent::AgentInputMode,
    permission_mode: PermissionMode,
    cwd: &std::path::Path,
) -> AgentOutput {
    let runner = SubprocessRunner {
        options: AgentRunOptions {
            permission_mode,
            input_mode,
            capture_output: true,
            stream_output: true,
        },
    };
    let mut spinner = display::ThinkingSpinner::start();
    let mut at_line_start = true;
    let mut first_chunk = true;

    let result = runner.run_streaming(prepared, cwd, |chunk| {
        use std::io::Write;
        if first_chunk {
            spinner.stop();
            first_chunk = false;
        }

        for ch in chunk.chars() {
            if at_line_start {
                print!("  > ");
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
            out
        }
        Err(e) => {
            display::agent_line(&format!("\x1b[91merror: {e}\x1b[0m"));
            AgentOutput {
                status: -1,
                stdout: String::new(),
                stderr: e.to_string(),
            }
        }
    }
}

fn write_snapshot(session: &Session, prefix: &str, snapshot: &GitSnapshot) -> Result<()> {
    session.write(
        &format!("{prefix}/head.txt"),
        &snapshot
            .head
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
    )?;
    session.write(
        &format!("{prefix}/branch.txt"),
        &snapshot
            .branch
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
    )?;
    session.write(
        &format!("{prefix}/status.porcelain.txt"),
        &snapshot.status_raw,
    )?;
    session.write(
        &format!("{prefix}/diff.name-status.txt"),
        &snapshot.diff_name_status,
    )?;
    Ok(())
}

fn handle_policy_violations(
    violations: &[policy::PolicyViolation],
    config: &Config,
    lang: &Language,
    agent: &str,
    role: &str,
) -> Result<bool> {
    use std::io::{self, Write};

    for violation in violations {
        match policy::approval_decision(&violation.event, config) {
            ApprovalDecision::Allow => {}
            ApprovalDecision::Deny => return Ok(false),
            ApprovalDecision::Ask => {
                println!();
                println!(
                    "  Approval required for {} ({}): {}",
                    display::agent_display(agent),
                    role,
                    violation.message
                );
                print!(
                    "  Continue this session? [{}] ",
                    match lang {
                        Language::Korean => "y/N",
                        Language::English => "y/N",
                    }
                );
                io::stdout().flush().ok();
                let mut input = String::new();
                io::stdin().read_line(&mut input).ok();
                let answer = input.trim().to_ascii_lowercase();
                if answer != "y" && answer != "yes" {
                    return Ok(false);
                }
            }
        }
    }

    Ok(true)
}

fn format_agent_output(agent: &str, role: &str, output: &AgentOutput) -> String {
    format!(
        "# {} ({}) Output\n\nExit status: {}\n\n## stdout\n\n{}\n\n## stderr\n\n{}\n",
        display::agent_display(agent),
        role,
        output.status,
        output.stdout,
        output.stderr
    )
}

fn format_step_meta(
    role: &str,
    agent: &str,
    command_preview: &str,
    prompt_file: &str,
    output: &AgentOutput,
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
        "role={role}\nagent={agent}\ncommand={command_preview}\nprompt_file={prompt_file}\nexit_code={}\nduration_ms={duration_ms}\nbranch_before={}\nbranch_after={}\nhead_before={}\nhead_after={}\nchanged_files=\n{}\n\npolicy_violations=\n{}\n",
        output.status,
        before.branch.as_deref().unwrap_or("unknown"),
        after.branch.as_deref().unwrap_or("unknown"),
        before.head.as_deref().unwrap_or("unknown"),
        after.head.as_deref().unwrap_or("unknown"),
        changed_files,
        violations
    )
}

fn format_policy_violation_error(
    agent: &str,
    role: &str,
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
            display::agent_display(agent),
            role,
            details
        ),
        &format!(
            "?뺤콉 ?꾨컲?쇰줈 {} ({}) ?④퀎 ?ㅽ뻾 ??以묐떒?덉뒿?덈떎: {}",
            display::agent_display(agent),
            role,
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

fn build_direct_agent_prompt(
    agent: &str,
    normalized_task_en: &str,
    original_task: &str,
    instructions_text: &str,
) -> String {
    format!(
        r#"You are running as a direct single-agent session in Wisp.

Agent: {agent}
Task: {normalized_task_en}
Original user input: {original_task}

Project instructions:
{instructions_text}

Rules:
- Do not hand off to another agent.
- If you need permission, ask the user directly in the terminal.
- Make minimal safe changes.
- Explain what you changed.
- Do not push.
- Do not commit unless the user explicitly approves it.
"#
    )
}

fn build_implement_prompt(
    normalized_task_en: &str,
    original_task: &str,
    instructions: &str,
) -> String {
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
- If requirements are ambiguous or risky, stop and request a user decision in the terminal.
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
- If the next action is unclear, request a user decision in the terminal.
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
"#,
        normalized_task_en, original_task
    )
}

fn finalize_workflow_summary(
    session: &Session,
    task: &str,
    normalized_task_en: &str,
    branch: &str,
    mode: &str,
    instructions: &LoadedInstructions,
    config: &Config,
) -> Result<()> {
    let final_snapshot = git::snapshot().context("Failed to capture final git snapshot")?;
    write_snapshot(session, "git/after", &final_snapshot)?;
    session.write(
        "summary.md",
        &format!(
            "# Wisp Session Summary\n\n## Task\n\nOriginal: {}\n\nNormalized: {}\n\n## Context\n\n- Branch: {}\n- Mode: {}\n- Session: {}\n- Instructions loaded: {} ({} bytes{})\n\n## Workflow\n\n1. {} implement\n2. {} patch\n3. {} review\n4. {} ship\n\n## Runtime Safety\n\n- Default interactive behavior is dry-run.\n- Protected branches block execution.\n- Dirty working tree blocks execution unless --allow-dirty is set.\n- Policy checks record changed files, HEAD movement, deletions, dependency changes, and protected path changes.\n",
            task,
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
        ),
    )
}

fn finalize_single_agent_summary(
    session: &Session,
    task: &str,
    normalized_task_en: &str,
    branch: &str,
    mode: &str,
    instructions: &LoadedInstructions,
    agent: &str,
) -> Result<()> {
    let final_snapshot = git::snapshot().context("Failed to capture final git snapshot")?;
    write_snapshot(session, "git/after", &final_snapshot)?;
    session.write(
        "summary.md",
        &format!(
            "# Wisp Single Agent Summary\n\n## Task\n\nOriginal: {}\n\nNormalized: {}\n\n## Context\n\n- Agent: {}\n- Branch: {}\n- Mode: {}\n- Session: {}\n- Instructions loaded: {} ({} bytes{})\n",
            task,
            normalized_task_en,
            agent,
            branch,
            mode,
            session.path().display(),
            instructions.files.len(),
            instructions.total_bytes,
            if instructions.truncated { ", truncated" } else { "" },
        ),
    )
}

fn mode_label(execute_agents: bool, permission_mode: PermissionMode) -> String {
    if !execute_agents {
        return "dry-run".to_string();
    }

    match permission_mode {
        PermissionMode::Interactive => "execute-interactive".to_string(),
        PermissionMode::Auto => "execute-auto".to_string(),
        PermissionMode::Skip => "execute-skip".to_string(),
    }
}
