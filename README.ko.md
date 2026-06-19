# Wisp

Wisp는 Claude CLI와 Codex CLI를 로컬에서 오케스트레이션하는 코딩 에이전트 런타임이다. 모든 실행 기록은 `.wisp/sessions/` 아래에 저장되고, 기본 동작은 안전한 dry-run이다.

## 기본 안전 정책

- 기본 모드는 dry-run이다.
- protected branch에서는 실제 실행을 막는다.
- 워킹 트리가 dirty 상태이면 `--allow-dirty` 없이는 실행하지 않는다.
- session에는 prompt, output, meta, git snapshot이 저장된다.
- 커밋 전에는 반드시 git diff를 검토한다.
- 중요한 브랜치에서는 `--execute-agents`를 쓰지 말고 feature branch를 사용한다.

## Interactive Mode

```text
task              dry-run preview only
!task             dry-run preview only
~task             dry-run preview only

/run task         full workflow execute
/exec task        full workflow execute

!claude task      ask Claude only
!codex task       ask Codex only

/run claude task  execute Claude only
/run codex task   execute Codex only

/auto task        execute workflow with auto permission mode
/auto claude task execute Claude only with auto permission mode
/auto codex task  execute Codex only with auto permission mode
```

`!claude`, `!codex`는 full workflow가 아니라 direct single-agent 세션이다.

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

## Permission Mode

- `interactive`: 터미널에서 사용자가 직접 Enter, `y`, `n` 등을 입력한다.
- `auto`: 설정된 auto-approve 인자를 agent CLI에 넘긴다.
- `skip`: 가능하면 권한이 필요한 실행을 피한다.

이제 기본 실행 경로는 child stdin에 prompt를 쓰고 닫는 방식이 아니다. prompt는 session 파일이나 command arg로 전달하고, agent stdin은 사용자 터미널에 연결해 둔다. 그래서 Claude/Codex가 실행 중 권한을 물으면 사용자가 직접 입력할 수 있다.

## 설정 예시

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

지원 placeholder:

- `{prompt}`
- `{prompt_file}`
- `{session_dir}`
- `{task}`

`input = "arg"`가 기본값이고, `input = "file"`도 지원한다.

## Session Layout

실행마다 다음과 같은 디렉터리가 생긴다.

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

single-agent 실행도 direct prompt, output, meta, before/after git snapshot을 같이 저장한다.

## PowerShell 예시

```powershell
wisp ask claude "이 코드 리뷰해줘" --execute-agents
wisp ask codex "테스트 추가해줘" --execute-agents
wisp ask codex "리팩토링해줘" --execute-agents --permission auto
wisp ask codex "분석만 해줘" --permission skip
```
