# 🦾 Anos — AI Native OS
> Trạng thái: 2026-05-24

## 📊 Tóm tắt

| Thông số | Giá trị |
|----------|---------|
| Version | **v0.4.0** |
| Branch | `dev_lor` |
| Releases | 7 tags (v0.1.0 → v0.4.0) |
| Code | 4,062 dòng Rust (13 module) |
| Tests | 8 passing |
| CI/CD | ✅ GitHub Actions (arm64 + x86_64) |

## ✅ 4 Phase hoàn thành

### Phase 1 — Core (v0.1.0)
- anosd daemon + anos-cli (Rust)
- SystemMap (live OS state graph)
- 6 AI Providers (hot-switch `/model`)
- 5 system tools
- Permission 4 levels
- Install script `curl | bash`

### Phase 2 — Intelligence (v0.2.1)
- IntentClassifier — 10 intents, confidence scoring
- Memory System — JSONL persistent, search, context
- AuditLogger — thread-safe, full trace
- SystemMap intent-filtering

### Phase 3 — Speed + Extensibility (v0.3.0)
- Sub-agent Spawn — background parallel tasks
- Hook System — 9 events, shell-based plugins

### Phase 4 — Safety + Evolution (v0.4.0)
- Snapshot System — btrfs auto-snapshot before dangerous ops
- Self-Upgrade — binary/source upgrade, auto-rollback

## 🔧 Commands

| Lệnh | Chức năng |
|------|-----------|
| `/model [id]` | Switch AI provider |
| `/providers` | List providers |
| `/tools` | List tools |
| `/memory` | Show memory |
| `/audit` | Show audit log |
| `/spawn <cmd>` | Run background task |
| `/agents` | List sub-agents |
| `/hooks` | List hooks |
| `/snapshot` | List snapshots |
| `/upgrade` | Check updates |

## 🔮 Tiếp theo

1. **ACP/Codex adapter** — future work (trung bình)
2. **gRPC streaming** — thay Unix socket (thấp)
3. **Qdrant Vector DB** — semantic memory upgrade (thấp)
4. **Desktop/TUI** — GUI app (thấp)

Tất cả 4 phase + 11 skill files đã hoàn thành ✅
