<div align="center">

# ✦ Wisp

**A local coding agent orchestrator. Claude implements. Codex ships. You stay in control.**

[![CI](https://github.com/LKA09/Wisp/actions/workflows/ci.yml/badge.svg)](https://github.com/LKA09/Wisp/actions/workflows/ci.yml)
[![CD](https://github.com/LKA09/Wisp/actions/workflows/release.yml/badge.svg?event=push)](https://github.com/LKA09/Wisp/actions/workflows/release.yml)[![npm](https://img.shields.io/npm/v/@lka09/wisp?color=a78bfa&label=npm)](https://www.npmjs.com/package/@lka09/wisp)
[![License: MIT](https://img.shields.io/badge/license-MIT-a78bfa.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-a78bfa.svg)](#installation)

**[한국어](README.ko.md)** · [Quick Start](#quick-start) · [How It Works](#how-it-works) · [Configuration](#configuration) · [Docs](#cli-reference)

</div>

---

Wisp is a **local-first agent orchestrator** that coordinates Claude and Codex through a structured 4-step workflow — implement → patch → review → ship — entirely on your machine, with no cloud dependency beyond the agents themselves.

Everything defaults to **dry-run preview**. Nothing changes in your repo until you say so. Every session is fully audited to `.wisp/sessions/`.

```
  ┌─ [1/4]  Claude  ─  implement    writes the solution
     [2/4]  Codex   ─  patch        reviews diff, applies fixes
     [3/4]  Claude  ─  review       APPROVED / CHANGES_REQUESTED
  └─ [4/4]  Codex   ─  ship         suggests commit message
```

---

## Quick Start

```sh
# Install (binary auto-downloaded from GitHub Releases)
npm install -g @lka09/wisp

# Initialize your project
cd your-project
wisp init

# Preview what agents would do — no files changed
wisp summon "add rate limiting to the API endpoints"

# Actually run it
wisp summon "add rate limiting to the API endpoints" --execute-agents
```

> **Prerequisite:** [Claude CLI](https://github.com/anthropics/claude-code) and/or [Codex CLI](https://github.com/openai/codex) installed and authenticated. Dry-run works without them.

---

## Why Wisp

| Problem | Wisp's answer |
|---|---|
| AI edits your files while you look away | Everything is dry-run by default |
| Can't tell what the agent actually did | Full session log — prompts, diffs, timing, policy results |
| One agent makes one big messy change | Four-role pipeline: implement → patch → review → ship |
| AI pushes commits you didn't approve | Hard policy block on commit and push |
| Agents ignore your project conventions | Load `.wisp/instructions.md` into every prompt |
| Korean task input breaks English-only tools | Auto-detects Korean, responds in Korean |

---

## How It Works

### Workflow

```
You: wisp summon "refactor the payment module"
      │
      ├─ 1. implement   Claude reads your task + project instructions,
      │                 writes the solution.
      │
      ├─ 2. patch       Codex reviews the diff, applies fixes.
      │                 (Repeats up to max_review_rounds times)
      │
      ├─ 3. review      Claude reviews the final diff.
      │                 → APPROVED  ·  CHANGES_REQUESTED  ·  NEEDS_USER_DECISION
      │
      └─ 4. ship        Codex prepares a commit message suggestion.
                        You decide whether to commit.
```

### Session audit trail

Every run writes a timestamped session under `.wisp/sessions/`:

```
.wisp/sessions/20260619-143022-123-p4801/
  task.original.txt              what you typed
  task.normalized.en.md          English translation for agents
  instructions.loaded.md         all project instruction files merged
  prompts/
    implementer.en.md            prompt sent to Claude
    patcher.en.md
    reviewer.en.md
    shipper.en.md
  outputs/
    implement.out.md             stdout + stderr
    implement.meta.txt           timing, exit code, git delta, policy checks
    implement.diff.before.patch  git state before this step
    implement.diff.after.patch   git state after this step
    patch.out.md  /  patch.meta.txt  /  ...
    review.out.md /  review.meta.txt /  ...
    ship.out.md   /  ship.meta.txt   /  ...
  git/
    before/  diff.patch  diff.cached.patch  status.porcelain.txt  ...
    after/   diff.patch  diff.cached.patch  status.porcelain.txt  ...
  summary.md
```

---

## Installation

### Option 1 — npm (recommended)

```sh
npm install -g @lka09/wisp
```

The postinstall script downloads the pre-built binary for your platform from GitHub Releases. If the download fails, it prints exact instructions for building from source.

**Supported platforms**

| OS | Architecture | Asset |
|---|---|---|
| Windows | x86_64 | `wisp-windows-x86_64.exe` |
| Windows | ARM64 | `wisp-windows-aarch64.exe` |
| Linux | x86_64 | `wisp-linux-x86_64` |
| Linux | ARM64 | `wisp-linux-aarch64` |
| macOS | x86_64 | `wisp-darwin-x86_64` |
| macOS | Apple Silicon | `wisp-darwin-aarch64` |

### Option 2 — Build from source

**Windows (PowerShell)**

```powershell
git clone https://github.com/LKA09/Wisp
cd Wisp\wisp
cargo build --release
New-Item -ItemType Directory -Force -Path ..\npm\dist | Out-Null
Copy-Item target\release\wisp.exe ..\npm\dist\wisp.exe
cd ..\npm
npm link
```

**Linux / macOS**

```sh
git clone https://github.com/LKA09/Wisp
cd Wisp/wisp
cargo build --release
mkdir -p ../npm/dist
cp target/release/wisp ../npm/dist/wisp
cd ../npm
npm link
```

### Prerequisites

| Tool | When needed | Install |
|---|---|---|
| Node.js ≥ 16 | Always | [nodejs.org](https://nodejs.org) |
| Git | Always | [git-scm.com](https://git-scm.com) |
| Rust + Cargo | Source build only | [rustup.rs](https://rustup.rs) |
| Claude CLI | `--execute-agents` | `npm i -g @anthropic-ai/claude-code` |
| Codex CLI | `--execute-agents` | `npm i -g @openai/codex` |

---

## Interactive Mode

Run `wisp` with no arguments to open the interactive REPL.

```
wisp
```

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ✦  Wisp  —  local coding agent
     Claude implements · Codex ships · you stay in control
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Type a task and press Enter — default is dry-run preview.
  Default is dry-run.  Use /run to execute  ·  exit to quit.

  › /
  ╭────────────────────────────────────────────────────────────╮
  │ /run       execute workflow interactively                  │
  │ /auto      execute workflow (auto-approve)                 │
  │ /claude    run Claude directly                             │
  │ /codex     run Codex directly                              │
  │ /paste     enter multi-line paste mode                     │
  │ /help      show commands                                   │
  │ /exit      exit wisp                                       │
  ╰────────────────────────────────────────────────────────────╯
```

Type `/` to open the live command picker. Completions filter as you type.

### Commands

| Input | Action |
|---|---|
| `<task>` | Dry-run preview (no files changed) |
| `/run <task>` | Execute full workflow interactively |
| `/auto <task>` | Execute full workflow (auto-approve) |
| `/claude <task>` | Run Claude as a single direct agent |
| `/codex <task>` | Run Codex as a single direct agent |
| `/paste` | Enter explicit multi-line paste mode |
| `/help` | Show help |
| `exit` / `quit` | Exit |

### Multi-line task input

**Auto-detection (lines mode)**
Paste a multi-line task and end with a trailing command on the last line:

```
fix the payment module
handle the edge case where currency is null
also update the tests
/run
```

| Trailing line | Effect |
|---|---|
| `/run` | Execute full workflow interactively |
| `/auto` | Execute full workflow (auto-approve) |
| `/claude` | Run Claude as single agent |
| `/codex` | Run Codex as single agent |
| *(none)* | Dry-run preview |

**Explicit paste mode (works on Windows PowerShell raw console)**

```
  › /paste
  [paste mode — type or paste content, end with /end on its own line]

  fix the payment module
  handle the edge case where currency is null
  also update the tests
  /end
  [pasted: 89 chars, 3 lines]

  command (/run  /auto  /claude  /codex  or Enter for dry-run)
  › /run
```

1. Type `/paste` → Enter
2. Paste your content
3. Type `/end` on its own line
4. Enter a command or press Enter for dry-run

---

## CLI Reference

```sh
# Project setup
wisp init                                           # create wisp.toml + .wisp/
wisp doctor                                         # check git, agents, config
wisp update                                         # update wisp to the latest version

# Workflow (4-step: implement → patch → review → ship)
wisp summon "<task>"                                # dry-run preview
wisp summon "<task>" --execute-agents               # execute
wisp summon "<task>" --execute-agents --allow-dirty
wisp summon "<task>" --execute-agents --permission auto

# Single agent
wisp ask claude "<task>"                            # dry-run
wisp ask claude "<task>" --execute-agents           # execute
wisp ask codex  "<task>" --execute-agents --permission auto
wisp ask codex  "<task>" --permission skip

# Info
wisp --help
wisp --version
wisp summon --help
```

**Flags**

| Flag | Default | Description |
|---|---|---|
| `--execute-agents` | off | Actually invoke agent CLIs |
| `--allow-dirty` | off | Skip uncommitted-changes check |
| `--permission interactive` | ✓ | Agent prompts user for approval |
| `--permission auto` | | Pass auto-approve flags to agent |
| `--permission skip` | | Skip permission-gated steps |

---

## Configuration

`wisp init` creates `wisp.toml` in your project root. Edit it to customize agents, workflow, and policy.

```toml
[language]
ui       = "auto"    # "auto" → detects Korean input, responds in Korean
fallback = "en"
internal = "en"

[agents.claude]
cmd   = "claude"
args  = ["-p", "{prompt}"]
input = "arg"
permission_interactive_args = []
permission_auto_args         = []
permission_skip_args         = []

[agents.codex]
cmd   = "codex"
args  = ["exec", "-s", "workspace-write", "{prompt}"]
input = "arg"
permission_interactive_args = []
permission_auto_args         = []
permission_skip_args         = []

[workflow]
implementer       = "claude"
patcher           = "codex"
reviewer          = "claude"
shipper           = "codex"
max_review_rounds = 2         # max patch/review retries before giving up

[approval]
push                        = "deny"   # always block push
commit                      = "ask"    # ask before commit
add_dependency              = "ask"
delete_file                 = "ask"
modify_protected_file       = "deny"
continue_after_test_failure = "ask"

[instructions]
files = [
  ".wisp/instructions.md",
  "WISP.md",
  "AGENTS.md",
  "AGENT.md",
  "CLAUDE.md",
  "CODEX.md",
]
max_bytes              = 32768
include_agent_specific = true

[policy]
protected_branches = ["main", "master"]
protected_paths    = [".env", ".env.local", ".git", "id_rsa", "secrets.toml", "credentials.json"]
deny_commands      = ["git push --force", "cargo publish", "npm publish", "rm -rf /"]
```

### Prompt placeholders

| Placeholder | Value |
|---|---|
| `{prompt}` | Full prompt text |
| `{prompt_file}` | Path to prompt file in the session directory |
| `{session_dir}` | Session directory path |
| `{task}` | Raw task string from the user |

### Project instructions

Create `.wisp/instructions.md` to inject project context into every agent prompt:

```markdown
# Project Instructions

- TypeScript + React 18. Use functional components and hooks only.
- Run `npm test` before considering any task complete.
- Never modify files under `src/generated/`.
- Commit messages must follow Conventional Commits.
```

`AGENTS.md`, `AGENT.md`, `CLAUDE.md`, `CODEX.md`, and `WISP.md` are loaded automatically if they exist.

---

## Safety Model

Wisp is built around the principle that **agents should suggest, not decide**.

| Guarantee | Mechanism |
|---|---|
| No changes without consent | Dry-run is the default; `--execute-agents` is explicit |
| Protected branches safe | Execution blocked on `main`, `master` (configurable) |
| Dirty tree protected | Blocked unless `--allow-dirty` is set |
| No surprise commits | Agents instructed never to `git commit` |
| No surprise pushes | `push` approval is `deny` by default in policy |
| Protected files safe | `deny_commands` and `protected_paths` in `wisp.toml` |
| Dependency changes flagged | `add_dependency` triggers approval gate |
| Full audit trail | Every session logged to `.wisp/sessions/` |

> **Note:** Wisp is not a security sandbox. Agents run with your full user permissions. The policy layer blocks specific commands and paths, but cannot prevent every possible unsafe action. Always review agent output before approving commits.

---

## Publishing

> This section is for maintainers who publish Wisp releases to GitHub and npm.

### Release order

1. **Bump versions** — only `wisp/Cargo.toml` needs to be updated manually:
   - `wisp/Cargo.toml` — `version = "x.y.z"`
   - `npm/package.json` — updated automatically by the CD workflow

2. **Run local validation:**

   ```sh
   cargo fmt --check --manifest-path wisp/Cargo.toml
   cargo clippy --manifest-path wisp/Cargo.toml -- -D warnings
   cargo test --manifest-path wisp/Cargo.toml
   npm pack --dry-run --prefix npm
   ```

3. **Create and push a version tag** (or trigger the Release workflow manually with the same tag):

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. **Confirm all GitHub Release assets are present** (the CD workflow handles npm publish automatically after this):
   - `wisp-windows-x86_64.exe`
   - `wisp-windows-aarch64.exe`
   - `wisp-linux-x86_64`
   - `wisp-linux-aarch64`
   - `wisp-darwin-x86_64`
   - `wisp-darwin-aarch64`

The CD workflow automatically publishes to npm once all GitHub Release assets are confirmed. No manual npm publish step needed.

### Important: publish order matters

The npm postinstall script is intentionally non-fatal — if the binary download fails it prints a source-build fallback message and exits cleanly. The CD workflow runs `npm publish` only after the GitHub Release job completes, so the assets are always available first.

### Windows ARM64 note

The `wisp-windows-aarch64.exe` asset requires a GitHub-hosted Windows ARM64 runner. If that runner type is unavailable at release time, omit the Windows ARM64 asset and either remove it from the supported-platforms table or mark it as source-build only.

---

## Contributing

```sh
git clone https://github.com/LKA09/Wisp
cd Wisp/wisp

cargo fmt
cargo clippy -- -D warnings
cargo test
```

- Code lives in `wisp/src/`
- No heavy dependencies — only `clap`, `serde`, `toml`, `anyhow`, `chrono`, `ureq`, `serde_json`
- PRs against `develop-ai` branch

---

## License

MIT © [LKA09](https://github.com/LKA09)
