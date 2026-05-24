# Changelog

## v0.8.1 — 2026-05-24

Patch: CLI now prints plain-text daemon responses.

### Fixed
- `anos-cli` previously only printed lines prefixed with `>> `, so slash commands like `/providers`, `/model`, `/checks`, `/watch`, `/memstatus` looked like they did nothing.
- CLI now prints raw/plain daemon lines too, while keeping tool/chat formatting.

## v0.8.0 — 2026-05-24

Phase 8: Real Qdrant Semantic Memory.

### Added
- 🧠 **Real Qdrant vector memory**
  - `QdrantClient` over Qdrant HTTP API
  - `QdrantConfig` from env: `ANOS_QDRANT_URL`, `QDRANT_URL`, `ANOS_QDRANT_COLLECTION`
  - Auto-create collection with Cosine distance and 384-dim vectors
  - Upsert memory entries into Qdrant points with full payload
  - Search Qdrant by vector similarity, reconstructing `MemoryEntry` hits
- 🧮 **Local hashing embeddings**
  - Deterministic 384-dimensional normalized vectors
  - No external embedding API required
  - JSONL fallback remains if Qdrant is down
- 🎛️ New commands
  - `/memstatus` — Qdrant status + fallback status
  - `/memindex` — index all JSONL memory into Qdrant
  - `/memsearch <query>` — Qdrant search first, JSONL fallback second
- 🔁 **Opportunistic Qdrant sync** after successful tool memory writes

### Changed
- `QdrantSemanticMemory` placeholder replaced with real HTTP client
- `SemanticHit.reason` now distinguishes Qdrant cosine similarity vs JSONL fallback

## v0.7.0 — 2026-05-24

Phase 7: Production Agent Hardening — alerts, persistence, semantic memory, streaming scaffold.

### Added
- 🚨 **Real watcher alerts**
  - Alerts are persisted to `watcher-alerts.jsonl`
  - `/alerts` shows recent watcher alerts
  - Alerts emit structured `StreamEventKind::Alert` frames in daemon logs
  - Severity support: Info, Warning, Critical
- 💾 **Persistent watcher config**
  - Watch state persists to `watcher.yaml`
  - `/watch on/off/threshold` survives daemon restarts
  - `/watch threshold <check> <value>` changes thresholds
- 🧠 **Semantic memory abstraction**
  - `SemanticMemory` trait
  - `JsonlSemanticMemory` lexical/tag-scored fallback
  - `QdrantSemanticMemory` placeholder behind trait for future backend
  - `/memsearch <query>` command
  - Prompt memory context now uses semantic-ranked hits when query is present
- 📡 **Streaming scaffold**
  - `StreamEvent` + `StreamEventKind` types
  - Supports START, DELTA, TOOL_START, TOOL_RESULT, ALERT, ERROR, END
  - `/stream` command documents current event protocol
  - Future SSE/gRPC can reuse same JSON event model

### Changed
- Watcher constructor now takes data dir and loads persisted config/alerts
- `/help` updated with `/alerts`, `/memsearch`, `/stream`

## v0.6.0 — 2026-05-24

Phase 6: Proactive Scheduling — Anos tự chạy checks định kỳ, không cần user trigger.

### Added
- 👁️ **Proactive Watcher** — background scheduler chạy health checks tự động
  - 6 built-in checks: disk, ram, updates, load, services, security
  - Mỗi check có interval riêng (5m - 6h), threshold configurable
  - Chạy trong `tokio::spawn` — không block daemon
- 🎛️ `/watch` — enable/disable checks
  - `/watch` — xem summary
  - `/watch on disk` — bật check disk
  - `/watch off updates` — tắt check updates
  - `/watch all` — bật tất cả
- 🎛️ `/checks` — list tất cả scheduled checks với status, interval, last value

### Built-in Checks
| Check | Interval | Threshold | Default |
|-------|----------|-----------|---------|
| 💾 Disk | 30min | 85% | ✅ On |
| 🧠 RAM | 15min | 90% | ✅ On |
| 📦 Updates | 6h | Any security | ✅ On |
| ⚡ Load | 10min | CPU×2 | ⚫ Off |
| 🔧 Services | 30min | Any down | ⚫ Off |
| 🛡️ Security | 1h | >10 failed, any ban | ⚫ Off |

## v0.5.0 — 2026-05-24

Phase 5: Agentic Loop — autonomous multi-step task execution.

### Added
- 🔁 **Agentic Loop** — Anos tự plan + execute + verify multi-step tasks
  - `/auto <goal>` — đưa goal, LLM tự lên plan, tự chạy từng bước, tự verify
  - `/auto confirm <goal>` — tự confirm dangerous steps (batch mode)
  - `AgenticEngine::plan()` — LLM sinh `ExecutionPlan` JSON với steps, tools, success criteria
  - `AgenticEngine::run()` — execute plan → verify → report
  - Auto-verify sau install/remove (check package info)
  - Max 5 steps per task, auto-fallback retry cho confirmation
- 🤖 `agentic.rs` — 290 line module

### How it works
```
User: /auto "cài neovim và check disk"
  → LLM plans: [1. search neovim, 2. install neovim, 3. disk_usage]
  → Execute step 1 → ✅ Found
  → Execute step 2 → ✅ Installed
  → Verify step 2 → ✅ Confirmed
  → Execute step 3 → ✅ 65% free
  → Report: 3/3 steps done in 2.1s
```

### Changed
- IPC: `/auto` + `/auto confirm` commands
- Help updated

## v0.4.0 — 2026-05-24

Phase 4: Snapshot Safety + Self-Upgrade.

### Added
- 📸 **Snapshot System** — automatic btrfs snapshots before dangerous tool executions
  - `SnapshotManager` with create, list, rollback, prune, status
  - Auto-snapshot before `process` and `package` tool calls
  - Rollback capability via `rollback(snapshot_id)`
  - Prune old snapshots to keep disk under control
  - `/snapshot` command to list current snapshots
- 🔄 **Self-Upgrade Tool** — Anos can upgrade itself
  - `/upgrade` — check GitHub releases for updates
  - Binary upgrade (download pre-built) + source build fallback
  - Auto-rollback on build failure (git reset)
  - Version detection from Cargo.toml
  - `restart_daemon()` to apply upgrades
  - Works with or without gh CLI (git tag fallback)

### Changed
- IPC: snapshot auto-created before dangerous tool execution
- IPC: `/snapshot` + `/upgrade` commands added
- CLI help updated

## v0.3.0 — 2026-05-24

Phase 3: Sub-agent Spawn System + Hook System.

### Added
- 🤖 **Sub-agent Spawn System** — background parallel task execution with status tracking
  - `/spawn <command>` — launch detached sub-agents
  - `/agents` — list all agents with status and output
  - `SubAgent` struct: id, name, task, status (Running/Completed/Failed/Killed), output
  - Non-blocking `tokio::spawn` + persistent JSONL storage
  - `AgentRegistry` with spawn, list, get, kill, stats
- 🪝 **Hook System** — extensible pre/post action hooks
  - 9 hook events: PreTool, PostTool, PreChat, PostChat, PreConfirm, PostConfirm, OnModelSwitch, OnSessionStart, OnSessionEnd
  - `/hooks` — list registered hooks
  - `HookRegistry` with load, register, remove, fire
  - Shell-based hooks with ANOS_HOOK_CONTEXT + ANOS_HOOK_NAME env vars
  - Auto-fire in IPC: PreChat + PreTool hooks active

### Changed
- IPC handler now 8-arity (process_chat takes hooks ref)
- CLI help updated with /spawn, /agents, /hooks

## v0.2.1 — 2026-05-24

Phase 2 completion: Intent Classifier, Memory System, Audit Logger.

### Added
- 🎯 **IntentClassifier** — proper intent classification with confidence scoring and 10 intent categories
- 🧠 **Memory System** — file-based persistent memory (JSONL) with search, record, and context injection
- 📋 **Audit Logger** — thread-safe audit trail logging all tool executions, permission checks, confirmations, and model switches
- 📊 **SystemMap filtering** — SystemMap now only includes sections relevant to the detected intent, saving tokens
- 🎛️ `/memory` command — show memory stats and recent entries
- 🎛️ `/audit` command — show audit log with real-time entries
- 🧪 7 unit tests (5 intent + 2 memory)

### Changed
- IPC handler fully integrated with Memory + AuditLogger + IntentClassifier
- System prompt now includes memory context alongside SystemMap
- Automatic memory recording for successful tool executions
- Tool confirmation flow fully audited
- CLI help updated with new commands

## v0.2.0 — 2026-05-23

Stable Phase 2 release promoted from `dev_lor`.

### Added
- 📁 **FileSystemTool** — `list`, `read`, `find`, `disk_usage`, `mkdir`, `write`
- 🌐 **NetworkTool** — `interfaces`, `listening_ports`, `routes`, `ping`, `dns_lookup`
- 🛡️ **Pending confirmation flow** for confirm-required/dangerous tools
- 🔁 **Agentic tool result loop** — tool outputs are fed back to the model for final natural-language answers
- 📦 **Binary-first installer** with source-build fallback
- 🏗️ **GitHub Actions CI + multi-arch release workflow**
  - `linux-arm64`
  - `linux-x86_64`

### Changed
- `/tools` now lists 5 tools: package, process, service, filesystem, network
- Installer supports `ANOS_BRANCH`, `ANOS_VERSION`, and arch-aware release downloads
- Provider docs now clarify OpenAI-compatible HTTP API contract
- Removed non-functional Codex/ACP placeholder from default providers; ACP adapter is future work
- Rust sources formatted and clippy-cleaned

### Verified
- `dev_lor` CI succeeded before promotion
- `anosd` fmt/clippy/test/build OK
- `anos-cli` fmt/clippy/test/build OK
- Release artifact workflow produced arm64 + x86_64 binaries for dev release
- Binary install path tested against `v0.1.2-dev.1`

## v0.1.2-dev.1 — 2026-05-23

Development prerelease from `dev_lor` for Phase 2 hardening.

### Added
- 🔁 **Agentic tool result loop** — tool outputs are fed back to the model so it can produce a final natural-language answer
- 📦 **Binary-first installer** — tries release binaries first, then falls back to user-space source build
- 🏗️ **Multi-arch release workflow** — builds/uploads `linux-arm64` and `linux-x86_64` release artifacts

### Changed
- 🧹 Removed non-functional Codex/ACP placeholder from default providers
- Clarified provider contract: default providers must be OpenAI-compatible HTTP APIs
- Installer supports `ANOS_VERSION` and arch-aware binary downloads

### Verified
- Latest `dev_lor` CI succeeded
- `anosd` fmt/clippy/test/build OK
- `anos-cli` fmt/clippy/test/build OK
- Binary install path tested against `v0.1.1-dev.1`

## v0.1.1-dev.1 — 2026-05-23

Development prerelease from `dev_lor`.

### Added
- 📁 **FileSystemTool** — `list`, `read`, `find`, `disk_usage`, `mkdir`, `write`
- 🌐 **NetworkTool** — `interfaces`, `listening_ports`, `routes`, `ping`, `dns_lookup`
- 🛡️ **Pending confirmation flow** for dangerous/confirm-required tools
  - `yes`, `y`, `ok`, `đồng ý`, `làm đi`, `confirm` execute pending action
  - `no`, `cancel`, `hủy`, `không` cancel pending action
- 🔧 OpenAI-compatible tool schemas are now sent in chat requests

### Changed
- `/tools` now lists 5 tools: package, process, service, filesystem, network
- README examples updated for filesystem and network usage
- Build warnings cleaned up in core modules
- Removed Codex/ACP placeholder from default provider config; future ACP support needs a dedicated adapter

### Verified
- `anosd` build OK
- `anos-cli` build OK
- Socket smoke tests: `/ping`, `/tools`

## v0.1.0 — 2026-05-23

Initial public release. First PoC with core features working.

### Added
- 🦾 `anosd` — Rust daemon with Unix socket IPC
- 💻 `anos-cli` — Interactive CLI with rustyline, colors, one-shot mode
- 🗺️ **SystemMap** — Live OS state graph (CPU, RAM, disk, processes)
- 🔌 **6 AI Providers** — DeepSeek, Claude, OpenAI, Ollama, Codex, Groq
- 🔄 **Hot-switch** `/model <id>` to change providers in real-time
- 🛠️ **5 System Tools** — Package, Process, Service, FileSystem, Network
- 🛡️ **Permission System** — 4 levels (ReadOnly, Safe, Confirm, Dangerous)
- 📋 **10 Domain Skills** — package, system, network, filesystem, process, kernel, security, self-upgrade, gui, provider
- 🇻🇳 **Vietnamese + English** natural language support
- 🚀 **Install script** — `curl | bash`, user-space only, zero sudo
- 📄 README with usage examples and troubleshooting

### Technical
- Architecture: 5-layer (Conversation → AI Brain → Provider → Tools → Kernel)
- Communication: Unix socket with streaming markers `[THINKING]/[END]`
- Providers: OpenAI-compatible API format (works with Ollama, vLLM, OpenRouter, etc.)
- Auto-loads API key from OpenClaw config or `ANOS_API_KEY` env var
- Built with Rust (daemon 7.2K lines, CLI 4.3K lines)
- MIT License
