# Changelog

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
