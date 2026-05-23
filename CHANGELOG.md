# Changelog

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
