# Wisp

**A local personal coding agent orchestrator for Claude CLI and Codex CLI.**

Wisp coordinates a 4-step agent workflow — implement → patch → review → ship — running entirely on your machine. Everything defaults to safe dry-run preview, with a full audit trail saved to `.wisp/sessions/`.

---

## Quick Start

```powershell
# 1. Initialize (creates wisp.toml and .wisp/)
wisp init

# 2. Verify environment
wisp doctor

# 3. Preview what agents would do (no changes made)
wisp summon "add input validation to the login form"

# 4. Actually execute
wisp summon "add input validation to the login form" --execute-agents
```

---

## How It Works

When you run `wisp summon`, four agents run in sequence:

```
  ┌─ [1/4]  Claude  —  implement   Write the solution
  └─ [2/4]  Codex   —  patch       Review diff, apply fixes
  └─ [3/4]  Claude  —  review      Code review, APPROVED / CHANGES_REQUESTED
  └─ [4/4]  Codex   —  ship        Suggest commit message
```

Every step is logged to `.wisp/sessions/YYYYMMDD-HHMMSS/` — prompts, outputs, git snapshots, timing, and policy checks.

---

## Interactive Mode

Run `wisp` with no arguments to enter the interactive REPL.

```
wisp
```

Type a task and press **Enter** to dry-run. Type **/** to see live command suggestions.

```
  ╭──────────────────────────────────────────────────────────╮
  │  /run <task>         execute workflow interactively      │
  │  /auto <task>        execute workflow (auto-approve)     │
  │  /claude <task>      run Claude directly                 │
  │  /codex <task>       run Codex directly                  │
  │  /help               show commands                       │
  │  /exit               exit wisp                          │
  ╰──────────────────────────────────────────────────────────╯
```

Completions filter as you type — `/r` shows only `/run`, `/cl` shows only `/claude`.

### Command reference

| Input | Action |
|---|---|
| `<task>` | Dry-run workflow (no changes) |
| `/run <task>` | Execute full workflow interactively |
| `/auto <task>` | Execute full workflow (auto-approve) |
| `/claude <task>` | Run Claude as a single direct agent |
| `/codex <task>` | Run Codex as a single direct agent |
| `/help` | Show command help |
| `exit` / `quit` | Exit Wisp |

---

## CLI Commands

```powershell
wisp init                                          # initialize project
wisp doctor                                        # check environment

wisp summon "<task>"                               # dry-run workflow
wisp summon "<task>" --execute-agents              # execute workflow
wisp summon "<task>" --execute-agents --permission auto

wisp ask claude "<task>"                           # dry-run single agent
wisp ask claude "<task>" --execute-agents          # execute single agent
wisp ask codex  "<task>" --execute-agents --permission auto
wisp ask codex  "<task>" --permission skip
```

### Flags

| Flag | Description |
|---|---|
| `--execute-agents` | Actually invoke agent CLIs (default: dry-run) |
| `--allow-dirty` | Allow running on a dirty working tree |
| `--permission interactive` | Agent can ask the user for approval (default) |
| `--permission auto` | Pass auto-approve args to the agent |
| `--permission skip` | Skip permission-requiring steps when possible |

---

## Installation

### Prerequisites

| Tool | Required | Install |
|---|---|---|
| Rust + Cargo | Build only | [rustup.rs](https://rustup.rs) |
| Node.js ≥ 16 | Always | [nodejs.org](https://nodejs.org) |
| Git | Always | [git-scm.com](https://git-scm.com) |
| Claude CLI | `--execute-agents` | `npm i -g @anthropic-ai/claude-code` |
| Codex CLI | `--execute-agents` | `npm i -g @openai/codex` |

### From source

```powershell
git clone https://github.com/LKA09/Wisp
cd Wisp

# Build and deploy in one step
.\build.ps1

# Install globally
cd npm && npm link
```

---

## Configuration — `wisp.toml`

Created automatically by `wisp init`. Edit to customize agents and policy.

```toml
[language]
ui       = "auto"   # "auto" detects Korean input and responds in Korean
fallback = "en"
internal = "en"

[agents.claude]
cmd  = "claude"
args = ["-p", "{prompt}"]
input = "arg"
permission_interactive_args = []
permission_auto_args         = []
permission_skip_args         = []

[agents.codex]
cmd  = "codex"
args = ["exec", "-s", "workspace-write", "{prompt}"]
input = "arg"
permission_interactive_args = []
permission_auto_args         = []
permission_skip_args         = []

[workflow]
implementer      = "claude"
patcher          = "codex"
reviewer         = "claude"
shipper          = "codex"
max_review_rounds = 2

[approval]
push                       = "deny"
commit                     = "ask"
add_dependency             = "ask"
delete_file                = "ask"
modify_protected_file      = "deny"
continue_after_test_failure = "ask"

[instructions]
files = [
  ".wisp/instructions.md",
  "WISP.md",
  "AGENTS.md",
  "CLAUDE.md",
  "CODEX.md"
]
max_bytes = 32768
include_agent_specific = true

[policy]
protected_branches = ["main", "master"]
protected_paths    = [".env", ".env.local", ".git", "id_rsa", "secrets.toml"]
deny_commands      = ["git push --force", "cargo publish", "npm publish", "rm -rf /"]
```

### Agent arg placeholders

| Placeholder | Value |
|---|---|
| `{prompt}` | Full prompt text |
| `{prompt_file}` | Path to prompt file written to the session |
| `{session_dir}` | Path to the current session directory |
| `{task}` | Raw task string from the user |

---

## Project Instructions

Wisp loads instruction files from your project before each workflow run. Create `.wisp/instructions.md` to give agents project-specific context:

```markdown
# Project Instructions

- This is a TypeScript/React project.
- Use functional components and hooks only.
- Run `npm test` before considering any task complete.
- Do not modify files in `src/generated/`.
```

Additional files (`AGENTS.md`, `CLAUDE.md`, `CODEX.md`, `WISP.md`) are loaded automatically if they exist.

---

## Session Layout

Each run creates a timestamped directory:

```
.wisp/sessions/20260619-143000/
  task.original.txt           original task as entered
  task.normalized.en.md       English-normalized task for agents
  instructions.loaded.md      all loaded project instructions
  prompts/
    implementer.en.md
    patcher.en.md
    reviewer.en.md
    shipper.en.md
  outputs/
    implement.out.md    stdout + stderr
    implement.meta.txt  timing, git delta, policy checks
    patch.out.md
    patch.meta.txt
    review.out.md
    review.meta.txt
    ship.out.md
    ship.meta.txt
  git/
    before/   head, branch, status, diff
    after/    head, branch, status, diff
  summary.md
```

---

## Safety Defaults

- **Dry-run by default.** No agent runs without `--execute-agents`.
- **Protected branches.** Execution is blocked on `main` and `master`.
- **Dirty tree check.** Blocked unless `--allow-dirty` is set.
- **Policy enforcement.** Denied commands, protected paths, and approval rules are evaluated after each agent step.
- **No auto-commit, no auto-push.** Agents are instructed never to commit or push. Wisp requires explicit user approval.
- **Korean language support.** If the task contains Korean, Wisp responds in Korean.

---

## License

MIT
