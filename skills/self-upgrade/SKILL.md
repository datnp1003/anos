---
name: self-upgrade
description: "Upgrade Anos daemon and CLI with rollback safety via Btrfs snapshot or git reset"
---

# Self-Upgrade Skill

You handle Anos self-evolution. Upgrade the daemon and CLI, with automatic rollback on failure.

## Available Commands

| Command | Description |
|---------|-------------|
| `/upgrade` | Check for available updates |
| `/upgrade source` | Build from source (git pull + cargo build) |
| `/snapshot` | List current snapshots |

## Workflow

### 1. "Kiểm tra update"
```
→ /upgrade
→ Checks GitHub releases for latest version
→ If newer: "v0.4.0 available (current: v0.3.0). Reply 'yes' to upgrade."
→ If current: "Already at latest version."
```

### 2. "Nâng cấp lên mới nhất"
```
→ Confirm: "Upgrading from v0.3.0 → v0.4.0"
→ Step 1: Try binary upgrade (download pre-built release)
→ If binary fails: fallback to source build
→ Step 2: git pull origin main
→ Step 3: cargo build --release
→ If build fails: auto-rollback (git reset --hard)
→ Report: "✅ Upgraded to v0.4.0" or "❌ Upgrade failed, rolled back"
```

### 3. "Nâng cấp từ source"
```
→ /upgrade source
→ git pull + cargo build --release
→ Builds both anosd and anos-cli
→ Auto-rollback on failure
```

## Upgrade Steps (Internal)

### Binary Upgrade Path
1. `gh release list` or `git tag` — find latest
2. `curl -fsSL <release-url>/anosd -o /tmp/anosd-new`
3. `chmod +x /tmp/anosd-new`
4. `/tmp/anosd-new --version` — verify
5. `cp /tmp/anosd-new <current-binary-path>`
6. Report success

### Source Upgrade Path
1. `git -C <anos-dir> pull origin main`
2. `cd <anos-dir>/anosd && cargo build --release`
3. If build fails → `git reset --hard HEAD~1`
4. `cd <anos-dir>/anos-cli && cargo build --release`
5. Report: new version + duration

### Snapshot Safety
If btrfs is available:
- Auto-create snapshot before source build
- If build fails → offer rollback via snapshot
- `/snapshot` to list available rollback points

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `gh: command not found` | gh CLI missing | Falls back to git tags |
| `git pull failed` | Network or merge conflict | Check internet, resolve manually |
| `cargo build failed` | Rust version or deps | Auto-rollback, report error |
| `binary verification failed` | Corrupt download | Falls back to source build |
| `Permission denied` | Binary path not writable | Check ~/.local/bin permissions |

## Safety Rules
- **ALWAYS** create snapshot before source build (if btrfs available)
- **ALWAYS** verify downloaded binary before replacing
- **ALWAYS** auto-rollback on build failure
- Never upgrade while a critical tool operation is in progress
- Warn user that daemon restart is needed to apply

## Vietnamese Keywords
- "nâng cấp", "upgrade", "update anos", "cập nhật" → upgrade
- "phiên bản", "version", "check" → check
- "source", "từ mã nguồn" → source build
- "rollback", "quay lại", "hoàn tác" → rollback
