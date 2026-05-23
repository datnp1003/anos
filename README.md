# 🦾 Anos — AI Native Operating System

Anos is an AI-native Linux OS where the AI doesn't just *run on* the OS — it **is** the OS. It can read, understand, optimize, upgrade, and heal your system through natural language conversation.

## ✨ Features

- 🗣️ **Natural Language** — Talk to your OS in Vietnamese or English
- 🗺️ **SystemMap** — Live OS state graph (CPU, RAM, disk, processes) fed to AI
- 🔧 **Tool System** — AI can actually DO things: install packages, manage processes, control services
- 🔌 **Multi-Provider** — DeepSeek, Claude, OpenAI, Ollama, Codex, Groq — hot-switch anytime
- 🎯 **Intent Routing** — System actions → local LLM, code → Codex, reasoning → Claude
- 🛡️ **Permission System** — 4 levels (ReadOnly, Safe, Confirm, Dangerous) with audit logging
- 🔄 **Self-Evolving** — AI can upgrade the OS, kernel, and itself

## 🚀 Install

```bash
curl -fsSL https://raw.githubusercontent.com/datnp1003/anos/main/install.sh | bash
```

> **Zero sudo, user-space only.** Installs to `~/.local/bin/`.

### Requirements

- Linux (any distro)
- Rust (auto-installed if missing)

## 🎮 Usage

```bash
# Interactive chat
anos

# One-shot
anos "Còn bao nhiêu disk trống?"
anos "Process nào tốn CPU nhất?"

# Pipe
echo "Check RAM" | anos
```

## ⌨️ Commands

| Command | Action |
|---------|--------|
| `/model` | Show current AI provider |
| `/model <id>` | Switch provider (deepseek, claude, openai...) |
| `/providers` | List all providers |
| `/tools` | List available tools |
| `/help` | Show help |
| `/exit` | Quit |

## 🛠️ Tools

| Tool | Actions | Permission |
|------|---------|------------|
| 📦 Package | search, install, remove, update, info | Confirm |
| ⚡ Process | list, kill, info | Dangerous |
| 🔄 Service | list, status, start, stop, restart, logs | Confirm |

## ⚙️ Configuration

### AI Providers

Edit `~/.anos/config/providers.yaml`:

```yaml
active: deepseek

providers:
  - id: my-provider
    name: My Custom Model
    type: openai-compatible
    endpoint: https://my-api.example.com/v1
    model: my-model-name
    api_key_env: ANOS_API_KEY
```

Any OpenAI-compatible API works — Ollama, vLLM, OpenRouter, Groq, etc.

### API Key

Anos auto-loads the API key from your OpenClaw config (`~/.openclaw/openclaw.json`).
Or set it manually:

```bash
export ANOS_API_KEY="<your-api-key>"
```

## 💡 Usage Examples

```bash
# System info
anos "Máy đang chạy sao rồi?"
anos "Còn bao nhiêu disk trống?"
anos "Process nào tốn CPU nhất?"

# Package management
anos "Cài Neovim"
anos "Có package nào cần update không?"

# Process management
anos "Kill process node đang leak RAM"
anos "Show tất cả process của user datnguyen"

# Services
anos "Restart nginx"
anos "Check log của anosd"

# Network
anos "Port nào đang mở?"
anos "Check DNS của datnp.com"

# Multi-provider
anos
> /model claude        # Switch to Claude
> /model deepseek      # Switch back
> /providers           # List all
```

## 🔧 Troubleshooting

| Problem | Solution |
|---------|----------|
| `Connection refused` | Daemon not running. Run `anosd &` manually |
| `401 Unauthorized` | Set `ANOS_API_KEY` env var |
| `Tool not found` | Run `/tools` to see available tools |
| `Permission denied` | Tool needs confirmation. Reply `yes` |

## 🏗️ Architecture

```
┌─────────────────────────────────────┐
│  🗣️ Conversation Layer              │
│  CLI • TUI • Desktop • Web          │
├─────────────────────────────────────┤
│  🧠 AI Brain (anosd)                │
│  Intent • Context • Memory • Tools  │
├─────────────────────────────────────┤
│  🔌 Provider Adapters               │
│  DeepSeek • Claude • OpenAI • ...   │
├─────────────────────────────────────┤
│  ⚙️ System Actions                  │
│  Package • Process • Service • ...  │
├─────────────────────────────────────┤
│  🐧 Linux Kernel                    │
│  /proc • /sys • eBPF • systemd     │
└─────────────────────────────────────┘
```

## 📦 Project Structure

```
anos/
├── ANOS-SYSTEM-PROMPT.md    # AI identity & rules
├── LICENSE                  # MIT
├── install.sh               # One-line installer
├── config/providers.yaml    # AI provider configuration
├── skills/                  # 10 domain skills
├── anosd/                   # Rust daemon
│   └── src/
│       ├── main.rs          # Entry point
│       ├── provider.rs      # Multi-provider registry
│       ├── context.rs       # Prompt + skill loader
│       ├── systemmap.rs     # Live OS state graph
│       ├── tools.rs         # Package, Process, Service tools
│       └── ipc.rs           # Unix socket IPC
└── anos-cli/                # Rust CLI client
    └── src/main.rs          # Interactive + one-shot
```

## 🔧 Development

```bash
# Build daemon
cd anosd && cargo build --release

# Build CLI
cd anos-cli && cargo build --release

# Or build all
cd anosd && cargo build --release && cd ../anos-cli && cargo build --release
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ANOS_DIR` | `~/.anos` | Skills + prompt directory |
| `ANOS_SOCKET` | `/tmp/anos.sock` | Daemon socket |
| `ANOS_API_KEY` | (from OpenClaw config) | API key for providers |

## 🌍 Vision

Anos is built to be the OS where **AI is not an app — AI is the foundation**. The OS is optimized for AI, and the AI can introspect and heal the OS. [Read the full architecture →](RESEARCH-ARCHITECTURE.md)

## 📄 License

MIT © 2026 Dat Nguyen

---

Made with 🦾 by [datnp1003](https://github.com/datnp1003)
