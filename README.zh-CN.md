<div align="center">

# Wisp

**本地优先的编码代理编排工具。Claude 负责实现，Codex 负责交付，控制权始终在你手中。**

[![CI](https://github.com/LKA09/Wisp/actions/workflows/ci.yml/badge.svg)](https://github.com/LKA09/Wisp/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/@lka09/wisp?color=a78bfa&label=npm)](https://www.npmjs.com/package/@lka09/wisp)
[![License: MIT](https://img.shields.io/badge/license-MIT-a78bfa.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-a78bfa.svg)](#安装)

**[English](README.md)** · **[한국어](README.ko.md)** · [快速开始](#快速开始) · [工作方式](#工作方式) · [配置](#配置) · [文档](#cli-参考)

</div>

---

Wisp 是一个**本地优先的代理编排工具**，通过结构化的 4 步工作流协调 Claude 和 Codex：`implement`、`patch`、`review`、`ship`。除代理本身外，不依赖任何云端服务，全部在你的机器上运行。

所有操作默认都是 **dry-run 预览**。在你明确允许之前，仓库不会被修改。每次会话都会完整记录到 `.wisp/sessions/`。

```
  [1/4]  Claude  ->  implement    编写方案
  [2/4]  Codex   ->  patch        审查 diff 并应用修复
  [3/4]  Claude  ->  review       APPROVED / CHANGES_REQUESTED
  [4/4]  Codex   ->  ship         生成提交信息建议
```

---

## 快速开始

```sh
# 安装（二进制会从 GitHub Releases 自动下载）
npm install -g @lka09/wisp

# 初始化项目
cd your-project
wisp init

# 预览代理会做什么，不修改文件
wisp summon "add rate limiting to the API endpoints"

# 实际执行
wisp summon "add rate limiting to the API endpoints" --execute-agents
```

> **前置要求：** 已安装并完成认证的 [Claude CLI](https://github.com/anthropics/claude-code) 和/或 [Codex CLI](https://github.com/openai/codex)。即使没有它们，dry-run 仍然可以工作。

---

## 为什么选择 Wisp

| 问题 | Wisp 的解决方式 |
|---|---|
| AI 在你没注意时直接修改文件 | 默认一切都是 dry-run |
| 无法知道代理到底做了什么 | 完整记录会话日志：提示词、diff、耗时、策略检查结果 |
| 单个代理一次性做出过大的混乱改动 | 四角色流水线：implement -> patch -> review -> ship |
| AI 擅自提交代码 | 通过策略默认阻止 commit 和 push |
| 代理忽略你的项目约定 | 每次都会加载 `.wisp/instructions.md` |
| 韩文任务输入和英文工具链不兼容 | 自动检测韩文并用韩文响应 |

---

## 工作方式

### 工作流

```
你: wisp summon "refactor the payment module"

1. implement   Claude 读取任务和项目说明，编写实现。
2. patch       Codex 审查 diff，并应用修复。
               （最多重复 max_review_rounds 次）
3. review      Claude 审查最终 diff。
               -> APPROVED / CHANGES_REQUESTED / NEEDS_USER_DECISION
4. ship        Codex 准备提交信息建议。
               是否提交由你决定。
```

### 会话审计记录

每次运行都会在 `.wisp/sessions/` 下生成带时间戳的会话目录：

```
.wisp/sessions/20260619-143022-123-p4801/
  task.original.txt              你输入的原始任务
  task.normalized.en.md          提供给代理的英文标准化版本
  instructions.loaded.md         已加载的全部项目说明文件
  prompts/
    implementer.en.md            发给 Claude 的提示词
    patcher.en.md
    reviewer.en.md
    shipper.en.md
  outputs/
    implement.out.md             stdout + stderr
    implement.meta.txt           耗时、退出码、git 变化、策略检查
    implement.diff.before.patch  此步骤前的 git 状态
    implement.diff.after.patch   此步骤后的 git 状态
    patch.out.md  /  patch.meta.txt  /  ...
    review.out.md /  review.meta.txt /  ...
    ship.out.md   /  ship.meta.txt   /  ...
  git/
    before/  diff.patch  diff.cached.patch  status.porcelain.txt  ...
    after/   diff.patch  diff.cached.patch  status.porcelain.txt  ...
  summary.md
```

---

## 安装

### 方式 1：npm（推荐）

```sh
npm install -g @lka09/wisp
```

安装脚本会从 GitHub Releases 自动下载适合你平台的预构建二进制文件。如果下载失败，会输出精确的源码构建说明。

**支持的平台**

| OS | 架构 | 资源文件 |
|---|---|---|
| Windows | x86_64 | `wisp-windows-x86_64.exe` |
| Windows | ARM64 | `wisp-windows-aarch64.exe` |
| Linux | x86_64 | `wisp-linux-x86_64` |
| Linux | ARM64 | `wisp-linux-aarch64` |
| macOS | x86_64 | `wisp-darwin-x86_64` |
| macOS | Apple Silicon | `wisp-darwin-aarch64` |

### 方式 2：从源码构建

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

### 前置依赖

| 工具 | 何时需要 | 安装方式 |
|---|---|---|
| Node.js >=16 | 始终需要 | [nodejs.org](https://nodejs.org) |
| Git | 始终需要 | [git-scm.com](https://git-scm.com) |
| Rust + Cargo | 仅源码构建 | [rustup.rs](https://rustup.rs) |
| Claude CLI | 使用 `--execute-agents` 时 | `npm i -g @anthropic-ai/claude-code` |
| Codex CLI | 使用 `--execute-agents` 时 | `npm i -g @openai/codex` |

---

## 交互模式

不带参数运行 `wisp` 会打开交互式 REPL：

```sh
wisp
```

输入 `/` 可以打开实时命令选择器，输入时会即时过滤补全结果。

### 命令

| 输入 | 动作 |
|---|---|
| `<task>` | 执行任务（遵循 `/mode` 设置，默认为 dry-run） |
| `/run <task>` | 交互式执行完整工作流 |
| `/auto <task>` | 自动批准并执行完整工作流 |
| `/claude <task>` | 仅运行 Claude |
| `/codex <task>` | 仅运行 Codex |
| `/mode [dry-run\|execute]` | 查看或设置默认执行模式 |
| `/paste` | 进入显式多行粘贴模式 |
| `/help` | 显示帮助 |
| `exit` / `quit` | 退出 |

### 默认模式

默认情况下，不带命令前缀（`/run`、`/auto` 等）直接输入的任务会以 dry-run 预览方式运行。使用 `/mode` 可以修改此行为：

```
  › /mode execute    # 之后裸任务输入将直接调用代理
  › /mode dry-run    # 恢复为仅预览模式（默认）
  › /mode            # 查看当前模式
```

该设置保存在 `.wisp/settings.toml` 中，跨会话持久生效。

### 多行任务输入

**自动检测（行模式）**

粘贴多行任务，并在最后一行输入命令：

```text
fix the payment module
handle the edge case where currency is null
also update the tests
/run
```

| 最后一行 | 效果 |
|---|---|
| `/run` | 交互式执行完整工作流 |
| `/auto` | 自动批准并执行完整工作流 |
| `/claude` | 仅运行 Claude |
| `/codex` | 仅运行 Codex |
| *(无)* | dry-run 预览 |

**显式粘贴模式（适用于 Windows PowerShell 原始控制台）**

```text
  /paste
  [paste mode - type or paste content, end with /end on its own line]

  fix the payment module
  handle the edge case where currency is null
  also update the tests
  /end
  [pasted: 89 chars, 3 lines]

  command (/run  /auto  /claude  /codex  or Enter for dry-run)
  /run
```

1. 输入 `/paste` 并按 Enter
2. 粘贴或输入内容
3. 单独一行输入 `/end`
4. 输入命令，或直接按 Enter 进行 dry-run

---

## CLI 参考

```sh
# 项目初始化
wisp init                                           # 创建 wisp.toml + .wisp/
wisp doctor                                         # 检查 git、代理、配置
wisp update                                         # 更新 wisp 到最新版本
wisp mode                                           # 查看当前默认模式
wisp mode dry-run                                   # 设置默认为 dry-run 预览
wisp mode execute                                   # 设置默认为执行代理

# 工作流（4 步：implement -> patch -> review -> ship）
wisp summon "<task>"                                # dry-run 预览
wisp summon "<task>" --execute-agents               # 执行
wisp summon "<task>" --execute-agents --allow-dirty
wisp summon "<task>" --execute-agents --permission auto

# 单代理模式
wisp ask claude "<task>"                            # dry-run
wisp ask claude "<task>" --execute-agents           # 执行
wisp ask codex  "<task>" --execute-agents --permission auto
wisp ask codex  "<task>" --permission skip

# 信息
wisp --help
wisp --version
wisp summon --help
```

**参数**

| 参数 | 默认值 | 说明 |
|---|---|---|
| `--execute-agents` | off | 实际调用代理 CLI |
| `--allow-dirty` | off | 跳过未提交变更检查 |
| `--permission interactive` | - | 代理向用户请求批准 |
| `--permission auto` | - | 传递自动批准参数 |
| `--permission skip` | - | 跳过需要权限的步骤 |

---

## 配置

`wisp init` 会在项目根目录创建 `wisp.toml`。你可以修改它来自定义代理、工作流和策略。

```toml
[language]
ui       = "auto"    # "auto" 会检测韩文输入并用韩文响应
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
max_review_rounds = 2         # patch/review 最大重试次数

[approval]
push                        = "deny"   # 始终阻止 push
commit                      = "ask"    # 提交前询问
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

### 提示词占位符

| 占位符 | 含义 |
|---|---|
| `{prompt}` | 完整提示词文本 |
| `{prompt_file}` | 会话目录中的提示词文件路径 |
| `{session_dir}` | 会话目录路径 |
| `{task}` | 用户输入的原始任务字符串 |

### 项目说明

创建 `.wisp/instructions.md` 后，其中内容会自动注入到每个代理提示词中：

```markdown
# Project Instructions

- TypeScript + React 18. 仅使用函数组件和 hooks。
- 任务完成前必须运行 `npm test`。
- 不要修改 `src/generated/` 下的文件。
- 提交信息必须遵循 Conventional Commits。
```

如果存在，`AGENTS.md`、`AGENT.md`、`CLAUDE.md`、`CODEX.md` 和 `WISP.md` 也会被自动加载。

---

## 安全模型

Wisp 的核心原则是：**代理负责建议，不负责决定。**

| 保证 | 机制 |
|---|---|
| 未经同意不修改 | 默认 dry-run；执行必须显式传入 `--execute-agents` |
| 保护分支安全 | 默认在 `main`、`master` 上阻止执行（可配置） |
| 保护脏工作区 | 未设置 `--allow-dirty` 时阻止执行 |
| 不会偷偷提交 | 明确指示代理不得执行 `git commit` |
| 不会偷偷推送 | 策略中 `push` 默认是 `deny` |
| 保护敏感文件 | 通过 `deny_commands` 和 `protected_paths` 约束 |
| 依赖变更会被标记 | `add_dependency` 会触发批准门控 |
| 完整审计记录 | 每次会话都记录到 `.wisp/sessions/` |

> **注意：** Wisp 不是安全沙箱。代理会使用你的用户权限运行。策略层可以阻止特定命令和路径，但无法防止所有潜在的不安全操作。批准提交前，请始终审查代理输出。

---

## 贡献

```sh
git clone https://github.com/LKA09/Wisp
cd Wisp/wisp

cargo fmt
cargo clippy -- -D warnings
cargo test
```

- 代码位于 `wisp/src/`
- 保持轻量依赖，只使用 `clap`、`serde`、`toml`、`anyhow`、`chrono`
- PR 请提交到 `develop-ai` 分支

---

## 许可证

MIT · [LKA09](https://github.com/LKA09)
