# Wisp

**Claude CLI와 Codex CLI를 로컬에서 실행하는 개인용 코딩 에이전트 오케스트레이터.**

Wisp는 implement → patch → review → ship 4단계 에이전트 워크플로우를 완전히 로컬 환경에서 실행한다. 기본 동작은 안전한 dry-run 미리보기이며, 모든 실행 기록은 `.wisp/sessions/`에 저장된다.

---

## 빠른 시작

```powershell
# 1. 초기화 (wisp.toml과 .wisp/ 생성)
wisp init

# 2. 환경 점검
wisp doctor

# 3. 에이전트가 할 일을 미리보기 (실제 변경 없음)
wisp summon "로그인 폼에 입력 검증 추가해줘"

# 4. 실제 실행
wisp summon "로그인 폼에 입력 검증 추가해줘" --execute-agents
```

---

## 동작 방식

`wisp summon`을 실행하면 4개의 에이전트가 순서대로 실행된다.

```
  ┌─ [1/4]  Claude  —  implement   솔루션 작성
  └─ [2/4]  Codex   —  patch       diff 검토 후 수정 적용
  └─ [3/4]  Claude  —  review      코드 리뷰 (APPROVED / CHANGES_REQUESTED)
  └─ [4/4]  Codex   —  ship        커밋 메시지 제안
```

모든 단계의 프롬프트, 출력, git 스냅샷, 실행 시간, 정책 검사 결과가 `.wisp/sessions/YYYYMMDD-HHMMSS/`에 기록된다.

---

## 인터랙티브 모드

인수 없이 `wisp`를 실행하면 인터랙티브 REPL이 시작된다.

```
wisp
```

작업을 입력하고 **Enter**를 누르면 dry-run이 실행된다. **/**를 입력하면 명령어 제안이 실시간으로 표시된다.

```
  ╭──────────────────────────────────────────────────────────╮
  │  /run <작업>         워크플로우 실행 (인터랙티브)          │
  │  /auto <작업>        워크플로우 실행 (자동 승인)           │
  │  /claude <작업>      Claude 단독 실행                     │
  │  /codex <작업>       Codex 단독 실행                      │
  │  /help               명령어 목록 표시                      │
  │  /exit               종료                                 │
  ╰──────────────────────────────────────────────────────────╯
```

타이핑할수록 제안이 필터링된다 — `/r`이면 `/run`만, `/cl`이면 `/claude`만 표시.

### 명령어 목록

| 입력 | 동작 |
|---|---|
| `<작업>` | Dry-run 워크플로우 (변경 없음) |
| `/run <작업>` | 전체 워크플로우 실행 (인터랙티브) |
| `/auto <작업>` | 전체 워크플로우 실행 (자동 승인) |
| `/claude <작업>` | Claude 단독 에이전트 실행 |
| `/codex <작업>` | Codex 단독 에이전트 실행 |
| `/help` | 명령어 도움말 |
| `exit` / `quit` | 종료 |

---

## CLI 명령어

```powershell
wisp init                                          # 프로젝트 초기화
wisp doctor                                        # 환경 점검

wisp summon "<작업>"                               # dry-run 워크플로우
wisp summon "<작업>" --execute-agents              # 워크플로우 실행
wisp summon "<작업>" --execute-agents --permission auto

wisp ask claude "<작업>"                           # Claude dry-run
wisp ask claude "<작업>" --execute-agents          # Claude 실행
wisp ask codex  "<작업>" --execute-agents --permission auto
wisp ask codex  "<작업>" --permission skip
```

### 플래그

| 플래그 | 설명 |
|---|---|
| `--execute-agents` | 에이전트 CLI를 실제로 실행 (기본값: dry-run) |
| `--allow-dirty` | 워킹 트리가 dirty 상태여도 실행 허용 |
| `--permission interactive` | 에이전트가 사용자에게 승인 요청 가능 (기본값) |
| `--permission auto` | 에이전트에 자동 승인 인자 전달 |
| `--permission skip` | 권한 필요 단계 건너뜀 |

---

## 설치

### 사전 요구사항

| 도구 | 필수 여부 | 설치 |
|---|---|---|
| Rust + Cargo | 빌드 시에만 | [rustup.rs](https://rustup.rs) |
| Node.js ≥ 16 | 항상 | [nodejs.org](https://nodejs.org) |
| Git | 항상 | [git-scm.com](https://git-scm.com) |
| Claude CLI | `--execute-agents` 사용 시 | `npm i -g @anthropic-ai/claude-code` |
| Codex CLI | `--execute-agents` 사용 시 | `npm i -g @openai/codex` |

### 소스에서 빌드

```powershell
git clone https://github.com/LKA09/Wisp
cd Wisp

# 빌드 + 배포 한 번에
.\build.ps1

# 전역 설치
cd npm && npm link
```

---

## 설정 — `wisp.toml`

`wisp init`이 자동으로 생성한다. 에이전트와 정책을 원하는 대로 수정할 수 있다.

```toml
[language]
ui       = "auto"   # "auto"는 한국어 입력을 감지해 한국어로 응답
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

### 에이전트 인자 플레이스홀더

| 플레이스홀더 | 값 |
|---|---|
| `{prompt}` | 전체 프롬프트 텍스트 |
| `{prompt_file}` | 세션 디렉터리에 저장된 프롬프트 파일 경로 |
| `{session_dir}` | 현재 세션 디렉터리 경로 |
| `{task}` | 사용자가 입력한 원본 작업 문자열 |

---

## 프로젝트 지시사항

Wisp는 워크플로우 실행 전에 프로젝트 지시사항 파일을 불러와 에이전트에 전달한다. `.wisp/instructions.md`를 만들어 프로젝트에 맞는 맥락을 제공할 수 있다.

```markdown
# 프로젝트 지시사항

- TypeScript/React 프로젝트다.
- 함수형 컴포넌트와 훅만 사용한다.
- 작업 완료 전 반드시 `npm test`를 실행한다.
- `src/generated/` 안의 파일은 수정하지 않는다.
```

`AGENTS.md`, `CLAUDE.md`, `CODEX.md`, `WISP.md`가 존재하면 자동으로 함께 불러온다.

---

## 세션 구조

실행마다 타임스탬프 디렉터리가 생성된다.

```
.wisp/sessions/20260619-143000/
  task.original.txt           사용자가 입력한 원본 작업
  task.normalized.en.md       에이전트용 영어 정규화 작업
  instructions.loaded.md      불러온 모든 프로젝트 지시사항
  prompts/
    implementer.en.md
    patcher.en.md
    reviewer.en.md
    shipper.en.md
  outputs/
    implement.out.md    stdout + stderr
    implement.meta.txt  실행 시간, git 변경사항, 정책 검사 결과
    patch.out.md / patch.meta.txt
    review.out.md / review.meta.txt
    ship.out.md / ship.meta.txt
  git/
    before/   head, branch, status, diff
    after/    head, branch, status, diff
  summary.md
```

---

## 기본 안전 정책

- **기본값은 dry-run.** `--execute-agents` 없이는 에이전트가 실행되지 않는다.
- **Protected branch 차단.** `main`, `master` 브랜치에서는 실행을 막는다.
- **Dirty tree 차단.** `--allow-dirty` 없이는 커밋되지 않은 변경사항이 있으면 실행하지 않는다.
- **정책 강제.** 각 에이전트 단계 후 금지 명령어, 보호 경로, 승인 규칙을 검사한다.
- **자동 커밋/푸시 없음.** 에이전트는 커밋이나 푸시를 하지 않도록 지시받는다. 사용자가 직접 승인해야 한다.
- **한국어 지원.** 작업에 한국어가 포함되면 Wisp가 한국어로 응답한다.

---

## 라이선스

MIT
