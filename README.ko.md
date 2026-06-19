<div align="center">

# ✦ Wisp

**로컬 개인 코딩 에이전트 오케스트레이터.**

*Claude가 구현하고, Codex가 배송하며, 결정은 당신이 내립니다.*

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![MVP](https://img.shields.io/badge/status-MVP-yellow.svg)]()

[English](./README.md)

</div>

---

## Wisp란?

Wisp는 당신과 AI 코딩 도구 사이에서 구조적이고 안전한 워크플로우로 두 CLI 에이전트를 조율합니다:

| 에이전트 | 역할 |
|---|---|
| **Claude CLI** | 구현자(Implementer) · 리뷰어(Reviewer) |
| **Codex CLI** | 패처(Patcher) · 시퍼(Shipper) |

모든 실행 내역은 로컬에 기록됩니다. 명시적인 승인 없이는 커밋도, 푸시도 하지 않습니다.

> **MVP 단계** — 핵심 구조, 설정, CLI, 세션 로깅, 워크플로우 스켈레톤이 모두 구현되어 있습니다.
> 실제 에이전트 호출은 `--execute-agents` 플래그와 Claude/Codex CLI 설치가 필요합니다.

---

## 빠른 시작

```bash
# 1. 빌드
cd wisp && cargo build --release

# 2. 프로젝트에 초기화
cd my-project
wisp init

# 3. 환경 점검
wisp doctor

# 4. 에이전트 소환 (기본값: 드라이런 — 안전)
wisp summon "인증 모듈에 에러 핸들링 추가해줘"
```

---

## 설치

### 요구 사항

| 도구 | 필수 | 비고 |
|---|---|---|
| [Rust](https://rustup.rs/) 1.75+ | 필수 | CLI 빌드용 |
| [Node.js](https://nodejs.org/) 16+ | 선택 | npm 래퍼 사용 시 |
| [Claude CLI](https://github.com/anthropics/claude-code) | 선택 | `--execute-agents` 사용 시 |
| [Codex CLI](https://github.com/openai/codex) | 선택 | `--execute-agents` 사용 시 |

### 소스에서 빌드

```bash
git clone https://github.com/lir09/wisp
cd wisp/wisp
cargo build --release
```

바이너리: `wisp/target/release/wisp` (Windows: `wisp.exe`)

### npm 래퍼 설치

```bash
# 바이너리를 npm 패키지에 복사
cp wisp/target/release/wisp npm/dist/        # Linux / macOS
copy wisp\target\release\wisp.exe npm\dist\  # Windows

# 전역 등록
cd npm && npm link
```

---

## 명령어

### `wisp`

```
Wisp

A local personal coding agent.

Usage:
  wisp init
  wisp doctor
  wisp summon "<task>"
```

### `wisp init`

현재 디렉토리에 Wisp를 초기화합니다.

```
Created wisp.toml
Created .wisp/sessions/
Created .wisp/instructions.md
```

```bash
wisp init           # 최초 설정
wisp init --force   # 기존 wisp.toml 덮어쓰기
```

### `wisp doctor`

환경을 점검하고 필요한 설치 방법을 안내합니다.

```
Wisp Doctor

  [OK  ] Git installed
  [OK  ] Git repository
  [FAIL] Claude CLI (claude)
      Install: npm install -g @anthropic-ai/claude-code
  [FAIL] Codex CLI (codex)
      Install: npm install -g @openai/codex
  [OK  ] wisp.toml exists
  [OK  ] .wisp/sessions/ exists

Note: Claude/Codex CLIs are optional for dry-run mode.
```

### `wisp summon "<task>"`

핵심 명령어. 작업에 대한 전체 에이전트 워크플로우를 실행합니다.

```bash
wisp summon "파서 테스트 코드 작성해줘"
wisp summon "README 정리해줘"                      # 한국어 입력 → 한국어 출력
wisp summon "인증 리팩토링" --allow-dirty           # 미커밋 변경사항 무시
wisp summon "인증 리팩토링" --execute-agents        # 실제 에이전트 실행
```

**드라이런 모드 (기본값)** — 안전하고 부작용 없음:

- 작업 언어 감지 (한국어 → 한국어 UI 출력)
- `wisp.toml` 및 프로젝트 지시사항 파일 로드
- 4개 에이전트 역할별 영문 프롬프트 생성
- 타임스탬프 세션 디렉토리에 프롬프트 + 플레이스홀더 출력 저장
- `git status` 및 `git diff` 캡처

**`--execute-agents`** — Claude와 Codex CLI를 실제로 호출합니다.

---

## 에이전트 워크플로우

```
wisp summon "작업"
      │
      ├─ 1. Claude  →  implement     (코드 작성)
      ├─ 2. Codex   →  patch         (최소 수정 적용)
      ├─ 3. Claude  →  review        (APPROVED / CHANGES_REQUESTED / NEEDS_USER_DECISION)
      └─ 4. Codex   →  ship          (커밋 메시지 제안 + 푸시 체크리스트)
                                      ↑
                              커밋·푸시 전 반드시
                              사용자 승인 필요
```

---

## 세션 구조

`wisp summon`을 실행할 때마다 완전한 감사 추적 디렉토리가 생성됩니다:

```
.wisp/sessions/20260619-143000/
├── task.original.txt           원본 작업 문자열
├── task.normalized.en.md       영문 정규화 작업
├── instructions.loaded.md      로드된 프로젝트 지시사항
│
├── prompts/
│   ├── claude.implement.en.md
│   ├── codex.patch.en.md
│   ├── claude.review.en.md
│   └── codex.ship.en.md
│
├── outputs/
│   ├── claude.implement.out.md
│   ├── codex.patch.out.md
│   ├── claude.review.out.md
│   └── codex.ship.out.md
│
├── git/
│   ├── status.txt
│   └── diff.patch
│
└── summary.md
```

---

## 설정 파일

`wisp.toml`은 `wisp init`이 생성합니다. 프로젝트에 맞게 수정하세요.

```toml
[agents.claude]
cmd  = "claude"
args = ["-p"]

[agents.codex]
cmd  = "codex"
args = ["exec"]

[workflow]
implementer      = "claude"
patcher          = "codex"
reviewer         = "claude"
shipper          = "codex"
max_review_rounds = 2

[approval]
push                       = "always"   # 명시적 승인 필요
commit                     = "ask"
add_dependency             = "ask"
delete_file                = "ask"
modify_protected_file      = "deny"
continue_after_test_failure = "ask"

[policy]
protected_paths = [".env", ".git", "id_rsa", "secrets.toml", "credentials.json"]
deny_commands   = ["git push --force", "rm -rf /", "cargo publish"]
```

### 프로젝트 지시사항

Wisp는 지시사항 파일을 자동으로 불러와 모든 에이전트 프롬프트에 주입합니다:

| 파일 | 용도 |
|---|---|
| `.wisp/instructions.md` | 일반 프로젝트 컨텍스트 |
| `WISP.md` | Wisp 전용 가이드 |
| `AGENTS.md` / `AGENT.md` | 에이전트 행동 규칙 |
| `CLAUDE.md` | Claude 전용 규칙 |
| `CODEX.md` | Codex 전용 규칙 |

---

## 안전 원칙

> Wisp는 기본적으로 안전하게 설계되어 있습니다.

- **승인 없이 push 불가** — `push = "always"`로 무감독 푸시 차단
- **위험 명령어 차단** — `git push --force`, `rm -rf /` 등 정책으로 금지
- **보호 파일 절대 수정 불가** — `.env`, `.git`, `id_rsa` 등
- **에이전트 의견 불일치 시 사용자에게 에스컬레이션** — Wisp는 임의로 선택하지 않음
- **내부 프롬프트는 항상 영어** — 사용자 메시지는 언어 자동 감지 후 현지화

---

## npm 래퍼 동작 방식

`npm/bin/wisp.js`는 순수 프로세스 래퍼입니다:

1. 플랫폼 감지 (`win32` → `wisp.exe`, 기타 → `wisp`)
2. `npm/dist/`에서 바이너리 경로 확인
3. `stdio: 'inherit'`으로 실행, 종료 코드 그대로 전달

실제 모든 로직은 Rust 바이너리 안에 있습니다.

---

## 라이선스

MIT
