# 🦾 Anos — AI Native Linux Agent

Anos is an AI-native control plane for Linux. It runs as a Rust daemon plus CLI and lets an operator inspect, monitor, repair, and automate a Linux host through natural language and explicit system tools.

> 🖥️ **Looking for the OS?** → [datnp1003/anos-os](https://github.com/datnp1003/anos-os) — bootable ISO, Docker image, multi-user login
>
> This repo is the **agent layer** (daemon + CLI + skills). The OS layer is in a separate repo for independent release cycles.

## Features

- **Natural-language operations** — Ask the host questions and request actions from the CLI.
- **Rust daemon + CLI** — `anosd` exposes a Unix socket; `anos-cli` provides interactive and one-shot usage.
- **Multi-provider AI** — OpenAI-compatible providers with hot switching via `/model`.
- **System tools** — Package, process, service, filesystem, and network tools.
- **Permission model** — Read-only, safe, confirmation-required, and dangerous actions.
- **Audit and memory** — JSONL audit/memory files for traceability and context.
- **Agentic loop** — Multi-step tool use with configurable loop limits and `/continue`.
- **Proactive watcher** — Periodic system checks with persisted alerts.
- **Semantic memory path** — JSONL fallback plus optional Qdrant indexing/search.
- **Setup and diagnostics** — `anos setup`, `anos status`, `anos doctor`, `anos policy`, and user systemd service generation.
- **SSE scaffold** — Health and event-stream endpoint for future streaming integrations.

## Install

Stable/main install:

```bash
curl -fsSL https://raw.githubusercontent.com/datnp1003/anos/main/install.sh | bash
```

Development branch install:

```bash
ANOS_BRANCH=dev_lor curl -fsSL https://raw.githubusercontent.com/datnp1003/anos/dev_lor/install.sh | bash
```

The default install is user-space only:

- Runtime: `~/.anos`
- Binaries: `~/.local/bin/anos`, `~/.local/bin/anosd`, `~/.local/bin/anos-cli`
- Socket: `/tmp/anos.sock`

### Upgrade and Config Preservation

The installer preserves user configuration and runtime state when it has to replace `~/.anos`:

- `config/providers.yaml`
- `policy.yaml`
- `memory.jsonl`
- `audit.jsonl`
- `watcher.yaml`
- `watcher-alerts.jsonl`
- `subagents.jsonl`
- `hooks.yaml`
- `qdrant/`

This means reinstalling or upgrading should not require provider setup again.

## Quick Start

```bash
# Check installation and permissions
anos doctor

# Configure provider and model
anos setup

# Show runtime status
anos status

# Start interactive CLI
anos

# One-shot examples
anos "How much disk space is free?"
anos "Which process is using the most CPU?"
```

## Shell Commands

These are handled by `anos-cli` directly and do not call the AI provider.

| Command | Purpose |
|---|---|
| `anos status` | Show daemon, socket, provider, policy, and service status |
| `anos doctor` | Run install, permission, and environment diagnostics |
| `anos setup` | Step-by-step provider configuration wizard |
| `anos install-service` | Write a user-level systemd unit for `anosd` |
| `anos policy` | Print the permission policy file |
| `anos policy init` | Create the default permission policy skeleton |
| `anos --version` | Print CLI version |

## Interactive Slash Commands

These are handled by the daemon/CLI and should not call the AI provider unless noted.

| Command | Purpose |
|---|---|
| `/help` | Show interactive help |
| `/version`, `/v`, `/versions` | Show daemon version and socket/SSE info |
| `/providers`, `/p` | List configured AI providers |
| `/model` | Show the active provider/model |
| `/model <id>` | Switch provider |
| `/tools` | List system tools |
| `/loop` | Show tool-loop limit and output mode |
| `/loop <1-20>` | Set per-session tool-loop limit |
| `/loop quiet` | Truncate long tool output in CLI |
| `/loop verbose` | Show full tool output in CLI |
| `/continue`, `/cont` | Continue from a saved tool-loop state |
| `/auto <goal>` | Run autonomous multi-step mode |
| `/watch` | Show proactive watcher summary |
| `/checks` | List scheduled checks |
| `/alerts` | Show recent watcher alerts |
| `/memstatus` | Show semantic memory backend status |
| `/memindex` | Index JSONL memory into Qdrant when available |
| `/memsearch <query>` | Search semantic memory |
| `/stream` | Show SSE streaming endpoint status |
| `/memory` | Show recent memory |
| `/audit` | Show recent audit records |
| `/spawn <cmd>` | Spawn a background sub-agent command |
| `/agents` | List spawned sub-agents |
| `/hooks` | List registered hooks |
| `/snapshot` | Show snapshot status/list |
| `/upgrade` | Check for Anos updates |
| `/ping` | Health check |
| `/exit`, `/quit` | Exit interactive CLI |

The interactive CLI supports command completion: type `/` and press `Tab`.

## System Tools

| Tool | Actions | Permission |
|---|---|---|
| `package` | `search`, `install`, `remove`, `update`, `info`, `list_upgradable` | Confirmation for changes |
| `process` | `list`, `info`, `kill`, `kill_by_name` | Dangerous for kill actions |
| `service` | `list`, `status`, `logs`, `start`, `stop`, `restart` | Confirmation for changes |
| `filesystem` | `list`, `read`, `find`, `disk_usage`, `mkdir`, `write` | Confirmation for writes |
| `network` | `interfaces`, `listening_ports`, `routes`, `ping`, `dns_lookup` | Read-only |

## Permission and Write Access

Anos does not automatically gain root access after installation. By default, it runs as the current user and can only write where that user can write, such as:

- `~/.anos`
- `/tmp`
- projects owned by the user

It cannot write to protected paths such as `/etc`, `/usr`, `/root`, or many `/var` locations unless the OS permissions allow it. This is intentional.

Create the default policy file:

```bash
anos policy init
```

Default location:

```text
~/.anos/policy.yaml
```

Example policy skeleton:

```yaml
mode: user
filesystem:
  allow_write:
    - /home/your-user
    - /tmp
  deny_write:
    - /etc
    - /root
    - /usr
commands:
  require_confirm:
    - apt install
    - apt remove
    - systemctl restart
    - systemctl stop
  deny:
    - rm -rf /
    - mkfs
    - dd if=
```

> The policy file currently provides the configuration skeleton. Runtime enforcement for every tool is the next hardening step.

## User Systemd Service

Generate a user service:

```bash
anos install-service
```

Then enable it:

```bash
systemctl --user enable --now anosd.service
```

Optional, to keep it running after logout:

```bash
loginctl enable-linger "$USER"
```

This is a user service, not a root service. It keeps the same write permissions as the user.

## Provider Configuration

Run the wizard:

```bash
anos setup
```

Or edit manually:

```text
~/.anos/config/providers.yaml
```

Example:

```yaml
active: 9router
providers:
  9router:
    name: 9Router
    baseUrl: https://9router.datnp.com/v1
    apiKeyEnv: ANOS_API_KEY
    model: cmc/deepseek/deepseek-v4-pro
```

Anos can also load a compatible 9router key from OpenClaw config when available.

## Qdrant Memory

Optional Qdrant setup:

```bash
docker run -d --name anos-qdrant \
  -p 6333:6333 \
  -v ~/.anos/qdrant:/qdrant/storage \
  qdrant/qdrant

export ANOS_QDRANT_URL=http://127.0.0.1:6333
export ANOS_QDRANT_COLLECTION=anos_memory
anos /memstatus
anos /memindex
anos /memsearch "web server failed to start"
```

Without Qdrant, Anos falls back to JSONL memory search.

## SSE Endpoint

By default, `anosd` exposes a minimal SSE server:

```text
http://127.0.0.1:8787/health
http://127.0.0.1:8787/events
```

Configure with:

```bash
export ANOS_SSE_ADDR=127.0.0.1:8787
export ANOS_SSE_DISABLE=1
```

Token streaming from providers is not fully wired yet; SSE currently provides health/start/heartbeat events and a transport foundation.

## Development

```bash
# Build daemon
cd anosd && cargo build --release

# Build CLI
cd anos-cli && cargo build --release

# Run daemon from source
ANOS_DIR="$PWD" ./anosd/target/release/anosd

# Use CLI from source
./anos-cli/target/release/anos-cli /providers
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `ANOS_DIR` | `~/.anos` | Runtime directory for prompt, skills, config, memory, audit, policy |
| `ANOS_SOCKET` | `/tmp/anos.sock` | Unix socket path |
| `ANOSD_BIN` | `~/.local/bin/anosd` | Daemon binary used by the wrapper |
| `ANOS_CLI_BIN` | `~/.local/bin/anos-cli` | CLI binary used by the wrapper |
| `ANOS_BIN_DIR` | `~/.local/bin` | Install binary directory |
| `ANOS_LOG` | `~/.anos/anosd.log` | Wrapper-started daemon log file |
| `ANOS_TOOL_LOOP_LIMIT` | `6` | Default tool-loop limit |
| `ANOS_TOOL_VERBOSE` | `0` | Show full tool output by default when set to `1` |
| `ANOS_QDRANT_URL` | `http://127.0.0.1:6333` | Qdrant endpoint |
| `ANOS_QDRANT_COLLECTION` | `anos_memory` | Qdrant collection |
| `ANOS_SSE_ADDR` | `127.0.0.1:8787` | SSE bind address |
| `ANOS_SSE_DISABLE` | unset | Disable SSE when set to `1` |

## Architecture

```text
┌─────────────────────────────────────┐
│ CLI / future TUI / future Desktop   │
├─────────────────────────────────────┤
│ anosd: Intent • Memory • Audit      │
├─────────────────────────────────────┤
│ Provider Adapters                   │
├─────────────────────────────────────┤
│ Tools: package/process/service/...  │
├─────────────────────────────────────┤
│ Linux Host: /proc /sys systemd      │
└─────────────────────────────────────┘
```

## Project Structure

```text
anos/
├── ANOS-SYSTEM-PROMPT.md
├── CHANGELOG.md
├── PROJECT-STATUS.md
├── README.md
├── install.sh
├── config/providers.yaml
├── skills/
├── anosd/
│   └── src/
│       ├── agentic.rs
│       ├── audit.rs
│       ├── context.rs
│       ├── hooks.rs
│       ├── intent.rs
│       ├── ipc.rs
│       ├── memory.rs
│       ├── provider.rs
│       ├── snapshot.rs
│       ├── spawn.rs
│       ├── streaming.rs
│       ├── sse.rs
│       ├── systemmap.rs
│       ├── tools.rs
│       ├── upgrade.rs
│       ├── vector_memory.rs
│       └── watcher.rs
└── anos-cli/
    └── src/main.rs
```

## Roadmap

Near-term hardening:

1. Enforce `policy.yaml` in tools.
2. Add persistent task and continuation state across daemon restarts.
3. Wire real provider token streaming into CLI/SSE.
4. Promote the latest development branch to `main` when stable.
5. Add integration tests with a mock provider.

## License

MIT © 2026 Dat Nguyen
