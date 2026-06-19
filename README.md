# Wisp

Wisp is a local coding agent orchestrator for Claude CLI and Codex CLI. It keeps an audit trail in `.wisp/sessions/` and defaults to safe dry-run behavior.

## Safety Defaults

- Default mode is safe dry-run.
- Interactive execution is blocked on protected branches.
- Execution is blocked on a dirty working tree unless `--allow-dirty` is set.
- Session logs include prompts, outputs, metadata, and git snapshots.
- Review the git diff before committing.
- Do not run `--execute-agents` on important branches. Use a feature branch.

## Interactive Mode

```text
task              dry-run preview only
/                 preview interactive commands
/help             show command help

/run task         full workflow execute
/exec task        full workflow execute

/claude task      ask Claude only
/codex task       ask Codex only

/run claude task  execute Claude only
/run codex task   execute Codex only

/auto task        execute workflow with auto permission mode
/auto claude task execute Claude only with auto permission mode
/auto codex task  execute Codex only with auto permission mode
```

`/claude` and `/codex` are direct single-agent sessions. They do not run the full implement/patch/review/ship workflow.

Legacy aliases `!claude`, `!codex`, `!task`, and `~task` still work for compatibility, but `/` is now the primary interactive command prefix.

## CLI

```bash
wisp init
wisp doctor
wisp summon "task"
wisp summon "task" --execute-agents
wisp ask claude "task"
wisp ask claude "task" --execute-agents
wisp ask codex "task" --execute-agents --permission auto
wisp ask codex "task" --permission skip
```

## Permission Modes

- `interactive`: user can approve prompts manually in the terminal.
- `auto`: pass configured auto-approve args to the agent.
- `skip`: avoid permission-requiring execution when possible.

Wisp no longer sends prompts by writing to child stdin and closing it as the default execution path. Prompts are stored in the session and passed by configured command arguments, while agent stdin stays attached to the user terminal. This lets Claude/Codex ask for Enter, `y`, or `n` during execution.

## Configuration

Example `wisp.toml`:

```toml
[agents.claude]
cmd = "claude"
args = ["-p", "{prompt}"]
input = "arg"
permission_interactive_args = []
permission_auto_args = []
permission_skip_args = []

[agents.codex]
cmd = "codex"
args = ["exec", "-s", "workspace-write", "{prompt}"]
input = "arg"
permission_interactive_args = []
permission_auto_args = []
permission_skip_args = []

[workflow]
implementer = "claude"
patcher = "codex"
reviewer = "claude"
shipper = "codex"
max_review_rounds = 2

[approval]
push = "deny"
commit = "ask"
add_dependency = "ask"
delete_file = "ask"
modify_protected_file = "deny"
continue_after_test_failure = "ask"

[policy]
protected_branches = ["main", "master"]
protected_paths = [".env", ".env.local", ".git", "id_rsa", "secrets.toml", "credentials.json"]
deny_commands = ["git push --force", "cargo publish", "npm publish", "rm -rf /"]
```

Supported placeholders in agent args:

- `{prompt}`
- `{prompt_file}`
- `{session_dir}`
- `{task}`

`input = "arg"` is the current default. `input = "file"` is also supported for prompt-file-based invocation.

## Session Layout

Each run creates a directory like:

```text
.wisp/sessions/20260619-143000/
  task.original.txt
  task.normalized.en.md
  instructions.loaded.md
  prompts/
  outputs/
  git/
  summary.md
```

Single-agent runs also store the direct prompt, output, metadata, and before/after git snapshots.

## PowerShell Examples

```powershell
wisp ask claude "이 코드 리뷰해줘" --execute-agents
wisp ask codex "테스트 추가해줘" --execute-agents
wisp ask codex "리팩토링해줘" --execute-agents --permission auto
wisp ask codex "분석만 해줘" --permission skip
```
