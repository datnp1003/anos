# 🦾 Anos — Project Status

Last updated: 2026-05-24

## Summary

| Item | Value |
|---|---|
| Latest tag | `v0.9.2` |
| Active branch | `dev_lor` |
| Current head | post-`v0.9.2` development commit with setup/status/doctor/service/policy CLI commands |
| Language | Rust |
| Runtime | Linux user-space daemon + CLI |
| Scope | AI-native Linux control plane, not a kernel/init replacement yet |

## Current Capabilities

- Rust daemon: `anosd`
- Rust CLI: `anos-cli`
- Wrapper command: `anos`
- Unix socket IPC: `/tmp/anos.sock`
- OpenAI-compatible provider registry
- Local slash commands that do not call AI
- System tools:
  - `package`
  - `process`
  - `service`
  - `filesystem`
  - `network`
- Intent classifier
- SystemMap context
- JSONL memory
- Qdrant-backed semantic memory path
- Audit log
- Hook registry
- Sub-agent spawn registry
- Btrfs snapshot helper
- Self-upgrade helper
- Proactive watcher and persisted alerts
- Agentic multi-step loop
- Configurable tool-loop limit
- `/continue` support after loop limit
- CLI slash-command completion
- Minimal SSE server
- CLI setup/status/doctor/service/policy commands

## Released Milestones

| Version | Summary |
|---|---|
| `v0.1.0` | Core daemon, CLI, SystemMap, providers, tools, installer |
| `v0.2.1` | Intent classifier, JSONL memory, audit logger |
| `v0.3.0` | Sub-agent spawn and hooks |
| `v0.4.0` | Snapshot and self-upgrade helpers |
| `v0.5.0` | Agentic multi-step loop |
| `v0.6.0` | Proactive watcher |
| `v0.7.0` | Watcher persistence, semantic memory abstraction, streaming scaffold |
| `v0.8.0` | Qdrant HTTP client and semantic memory commands |
| `v0.8.1` | Fixed CLI output swallowing slash-command responses |
| `v0.8.2` | Fixed 9router/OpenClaw API key loading |
| `v0.8.3` | Fixed installed runtime assets and daemon log path |
| `v0.8.4` | Added `/version` and `/v` |
| `v0.9.0` | Added `/versions`, unknown slash-command guard, minimal SSE server |
| `v0.9.1` | Configurable tool-loop limit, quiet/verbose output, `/continue`, CLI completions |
| `v0.9.2` | Fixed immediate `/exit` behavior |

## Post-v0.9.2 Development

Implemented but not released yet:

- `anos status`
- `anos doctor`
- `anos setup`
- `anos install-service`
- `anos policy`
- `anos policy init`

Commit:

```text
60007e0 Add CLI setup, doctor, status, service and policy commands
```

## Important Branch Note

Latest development is on `dev_lor`.

The `main` branch may lag behind until explicitly promoted. The command below installs whatever is on `main`, not necessarily the latest development build:

```bash
curl -fsSL https://raw.githubusercontent.com/datnp1003/anos/main/install.sh | bash
```

Use this for the development branch:

```bash
ANOS_BRANCH=dev_lor curl -fsSL https://raw.githubusercontent.com/datnp1003/anos/dev_lor/install.sh | bash
```

## Permission Model Status

Current default mode is user-space.

- Anos can write where the current user can write.
- Anos cannot write to `/etc`, `/usr`, `/root`, or protected system paths unless the OS permissions allow it.
- `anos policy init` creates `~/.anos/policy.yaml`.
- Full runtime enforcement of `policy.yaml` is still pending.

## Next Hardening Steps

1. Enforce `policy.yaml` in `filesystem`, `package`, `service`, and `process` tools.
2. Add persistent task and continuation state across daemon restarts.
3. Add real provider token streaming to CLI/SSE.
4. Add non-interactive setup flags for automation.
5. Add integration tests with a mock provider and temporary `ANOS_DIR`.
6. Promote `dev_lor` to `main` after verification.
