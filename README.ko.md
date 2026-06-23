<div align="center">

# ✦ Wisp

**로컬 코딩 에이전트 오케스트레이터. Claude가 구현하고, Codex가 배포한다. 결정권은 당신에게 있다.**

[![CI](https://github.com/LKA09/Wisp/actions/workflows/ci.yml/badge.svg)](https://github.com/LKA09/Wisp/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/@lka09/wisp?color=a78bfa&label=npm)](https://www.npmjs.com/package/@lka09/wisp)
[![License: MIT](https://img.shields.io/badge/license-MIT-a78bfa.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-a78bfa.svg)](#설치)

**[English](README.md)** · **[简体中文](README.zh-CN.md)** · [빠른 시작](#빠른-시작) · [동작 방식](#동작-방식) · [설정](#설정) · [CLI 레퍼런스](#cli-레퍼런스)

</div>

---

Wisp는 **로컬 우선 에이전트 오케스트레이터**다. Claude와 Codex를 implement → patch → review → ship 4단계 파이프라인으로 조율하고, 클라우드 의존 없이 전부 내 컴퓨터에서 실행된다.

기본 동작은 항상 **dry-run 미리보기**다. 내가 승인하기 전까지 내 저장소에는 아무것도 바뀌지 않는다. 모든 세션은 `.wisp/sessions/`에 완전히 기록된다.

```
  ┌─ [1/4]  Claude  ─  implement    솔루션 작성
     [2/4]  Codex   ─  patch        diff 검토 후 수정 적용
     [3/4]  Claude  ─  review       APPROVED / CHANGES_REQUESTED
  └─ [4/4]  Codex   ─  ship         커밋 메시지 제안
```

---

## 빠른 시작

```sh
# 설치 (GitHub Releases에서 플랫폼별 바이너리 자동 다운로드)
npm install -g @lka09/wisp

# 프로젝트 초기화
cd 내-프로젝트
wisp init

# 에이전트가 뭘 할지 미리보기 — 파일 변경 없음
wisp summon "API 엔드포인트에 rate limiting 추가해줘"

# 실제로 실행
wisp summon "API 엔드포인트에 rate limiting 추가해줘" --execute-agents
```

> **사전 조건:** [Claude CLI](https://github.com/anthropics/claude-code) 또는 [Codex CLI](https://github.com/openai/codex) 설치 및 인증 필요. dry-run은 에이전트 없이도 동작한다.

---

## 왜 Wisp를 쓰는가

| 문제 | Wisp의 답 |
|---|---|
| AI가 내가 모르는 사이에 파일을 수정한다 | 기본값은 dry-run — 내가 명시적으로 허용해야 변경된다 |
| 에이전트가 실제로 뭘 했는지 알 수 없다 | 세션 전체 로그 — 프롬프트, diff, 실행 시간, 정책 검사 결과 |
| 에이전트 하나가 한 번에 너무 많이 바꾼다 | 4단계 파이프라인으로 각 역할을 분리 |
| 에이전트가 커밋을 마음대로 올린다 | 커밋과 푸시는 정책으로 차단 |
| 에이전트가 프로젝트 규칙을 무시한다 | `.wisp/instructions.md`를 모든 프롬프트에 주입 |
| 한국어 입력이 영어 전용 툴에서 제대로 안 된다 | 한국어 자동 감지, 한국어로 응답 |

---

## 동작 방식

### 워크플로우

```
나: wisp summon "결제 모듈 리팩토링해줘"
    │
    ├─ 1. implement   Claude가 작업 + 프로젝트 지시사항을 읽고
    │                 솔루션을 작성한다.
    │
    ├─ 2. patch       Codex가 diff를 검토하고 수정 사항을 적용한다.
    │                 (최대 max_review_rounds 회 반복)
    │
    ├─ 3. review      Claude가 최종 diff를 리뷰한다.
    │                 → APPROVED  ·  CHANGES_REQUESTED  ·  NEEDS_USER_DECISION
    │
    └─ 4. ship        Codex가 커밋 메시지 초안을 제안한다.
                      커밋 여부는 내가 결정한다.
```

### 세션 감사 기록

모든 실행마다 `.wisp/sessions/` 아래에 타임스탬프 디렉터리가 생성된다.

```
.wisp/sessions/20260619-143022-123-p4801/
  task.original.txt              내가 입력한 원본 작업
  task.normalized.en.md          에이전트용 영어 번역
  instructions.loaded.md         불러온 프로젝트 지시사항 전체
  prompts/
    implementer.en.md            Claude에게 보낸 프롬프트
    patcher.en.md
    reviewer.en.md
    shipper.en.md
  outputs/
    implement.out.md             stdout + stderr
    implement.meta.txt           실행 시간, 종료 코드, git 변경사항, 정책 검사
    implement.diff.before.patch  이 단계 실행 전 git 상태
    implement.diff.after.patch   이 단계 실행 후 git 상태
    patch.out.md  /  patch.meta.txt  /  ...
    review.out.md /  review.meta.txt /  ...
    ship.out.md   /  ship.meta.txt   /  ...
  git/
    before/  diff.patch  diff.cached.patch  status.porcelain.txt  ...
    after/   diff.patch  diff.cached.patch  status.porcelain.txt  ...
  summary.md
```

---

## 설치

### 방법 1 — npm (권장)

```sh
npm install -g @lka09/wisp
```

postinstall 스크립트가 GitHub Releases에서 플랫폼에 맞는 바이너리를 자동으로 다운로드한다. 다운로드에 실패하면 소스 빌드 방법을 정확하게 출력해준다.

**지원 플랫폼**

| OS | 아키텍처 | 릴리스 에셋 |
|---|---|---|
| Windows | x86_64 | `wisp-windows-x86_64.exe` |
| Windows | ARM64 | `wisp-windows-aarch64.exe` |
| Linux | x86_64 | `wisp-linux-x86_64` |
| Linux | ARM64 | `wisp-linux-aarch64` |
| macOS | x86_64 | `wisp-darwin-x86_64` |
| macOS | Apple Silicon | `wisp-darwin-aarch64` |

### 방법 2 — 소스 빌드

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

### 사전 요구사항

| 도구 | 필요 시점 | 설치 |
|---|---|---|
| Node.js ≥ 16 | 항상 | [nodejs.org](https://nodejs.org) |
| Git | 항상 | [git-scm.com](https://git-scm.com) |
| Rust + Cargo | 소스 빌드 시 | [rustup.rs](https://rustup.rs) |
| Claude CLI | `--execute-agents` 사용 시 | `npm i -g @anthropic-ai/claude-code` |
| Codex CLI | `--execute-agents` 사용 시 | `npm i -g @openai/codex` |

---

## 인터랙티브 모드

인수 없이 `wisp`를 실행하면 인터랙티브 REPL이 열린다.

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

`/`를 입력하면 명령어 선택 창이 열린다. 타이핑할수록 실시간으로 필터링된다.

### 명령어

| 입력 | 동작 |
|---|---|
| `<작업>` | Dry-run 미리보기 (파일 변경 없음) |
| `/run <작업>` | 전체 워크플로우 실행 (인터랙티브) |
| `/auto <작업>` | 전체 워크플로우 실행 (자동 승인) |
| `/claude <작업>` | Claude 단독 에이전트 실행 |
| `/codex <작업>` | Codex 단독 에이전트 실행 |
| `/paste` | 여러 줄 붙여넣기 모드 |
| `/help` | 도움말 |
| `exit` / `quit` | 종료 |

### 여러 줄 작업 입력

**자동 감지 (라인 모드)**

여러 줄로 된 작업을 붙여넣고 마지막 줄에 명령어를 입력하면 된다.

```
결제 모듈 리팩토링해줘
currency가 null인 엣지 케이스 처리 추가하고
테스트도 같이 업데이트해줘
/run
```

| 마지막 줄 | 동작 |
|---|---|
| `/run` | 전체 워크플로우 실행 (인터랙티브) |
| `/auto` | 전체 워크플로우 실행 (자동 승인) |
| `/claude` | Claude 단독 에이전트 실행 |
| `/codex` | Codex 단독 에이전트 실행 |
| *(없음)* | Dry-run 미리보기 |

**명시적 붙여넣기 모드 (Windows PowerShell 포함 모든 터미널에서 동작)**

```
  › /paste
  [paste mode — type or paste content, end with /end on its own line]

  결제 모듈 리팩토링해줘
  currency가 null인 엣지 케이스 처리 추가하고
  테스트도 같이 업데이트해줘
  /end
  [pasted: 72 chars, 3 lines]

  command (/run  /auto  /claude  /codex  or Enter for dry-run)
  › /run
```

1. `/paste` 입력 → Enter
2. 내용을 붙여넣거나 입력
3. 독립된 줄에 `/end` 입력
4. 명령어 입력 또는 Enter (dry-run)

---

## CLI 레퍼런스

```sh
# 프로젝트 설정
wisp init                                           # wisp.toml + .wisp/ 생성
wisp doctor                                         # git, 에이전트, 설정 확인

# 워크플로우 (4단계: implement → patch → review → ship)
wisp summon "<작업>"                                # dry-run 미리보기
wisp summon "<작업>" --execute-agents               # 실행
wisp summon "<작업>" --execute-agents --allow-dirty
wisp summon "<작업>" --execute-agents --permission auto

# 단일 에이전트
wisp ask claude "<작업>"                            # dry-run
wisp ask claude "<작업>" --execute-agents           # 실행
wisp ask codex  "<작업>" --execute-agents --permission auto
wisp ask codex  "<작업>" --permission skip

# 정보
wisp --help
wisp --version
wisp summon --help
```

**플래그**

| 플래그 | 기본값 | 설명 |
|---|---|---|
| `--execute-agents` | 꺼짐 | 에이전트 CLI 실제 실행 |
| `--allow-dirty` | 꺼짐 | 커밋되지 않은 변경사항 허용 |
| `--permission interactive` | ✓ | 에이전트가 사용자에게 승인 요청 |
| `--permission auto` | | 자동 승인 플래그 전달 |
| `--permission skip` | | 권한 필요 단계 건너뜀 |

---

## 설정

`wisp init`이 프로젝트 루트에 `wisp.toml`을 자동으로 생성한다.

```toml
[language]
ui       = "auto"    # "auto" → 한국어 입력 감지 시 한국어로 응답
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
max_review_rounds = 2         # patch/review 최대 재시도 횟수

[approval]
push                        = "deny"   # 푸시는 항상 차단
commit                      = "ask"    # 커밋 전 확인
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

### 프롬프트 플레이스홀더

| 플레이스홀더 | 값 |
|---|---|
| `{prompt}` | 전체 프롬프트 텍스트 |
| `{prompt_file}` | 세션 디렉터리의 프롬프트 파일 경로 |
| `{session_dir}` | 세션 디렉터리 경로 |
| `{task}` | 사용자가 입력한 원본 작업 문자열 |

### 프로젝트 지시사항

`.wisp/instructions.md`를 만들면 모든 에이전트 프롬프트에 프로젝트 맥락이 자동으로 주입된다.

```markdown
# 프로젝트 지시사항

- TypeScript + React 18 프로젝트다. 함수형 컴포넌트와 훅만 사용한다.
- 작업 완료 전 반드시 `npm test`를 실행한다.
- `src/generated/` 안의 파일은 수정하지 않는다.
- 커밋 메시지는 Conventional Commits 형식을 따른다.
```

`AGENTS.md`, `AGENT.md`, `CLAUDE.md`, `CODEX.md`, `WISP.md`가 있으면 자동으로 함께 불러온다.

---

## 안전 모델

Wisp는 **에이전트는 제안하고, 결정은 사람이 한다**는 원칙 위에서 만들어졌다.

| 보장 | 방법 |
|---|---|
| 동의 없이 변경 없음 | dry-run이 기본값. `--execute-agents`는 명시적으로 설정해야 함 |
| protected 브랜치 보호 | `main`, `master`에서 실행 차단 (설정 가능) |
| dirty tree 보호 | `--allow-dirty` 없으면 차단 |
| 깜짝 커밋 없음 | 에이전트는 `git commit` 금지 지시를 받음 |
| 깜짝 푸시 없음 | 정책에서 `push`의 기본값은 `deny` |
| 보호된 파일 안전 | `deny_commands`와 `protected_paths`로 차단 |
| 의존성 변경 알림 | `add_dependency`는 승인 게이트를 통과해야 함 |
| 완전한 감사 기록 | 모든 세션이 `.wisp/sessions/`에 기록 |

> **주의:** Wisp는 보안 샌드박스가 아니다. 에이전트는 내 사용자 권한으로 실행된다. 정책 레이어가 특정 명령어와 경로를 차단하지만 모든 위험한 동작을 막을 수는 없다. 커밋을 승인하기 전에 에이전트 출력을 반드시 검토하자.

---

## 기여

```sh
git clone https://github.com/LKA09/Wisp
cd Wisp/wisp

cargo fmt
cargo clippy -- -D warnings
cargo test
```

- 코드는 `wisp/src/`에 있다
- 무거운 의존성 없음 — `clap`, `serde`, `toml`, `anyhow`, `chrono`만 사용
- PR은 `develop-ai` 브랜치로

---

## 라이선스

MIT © [LKA09](https://github.com/LKA09)
