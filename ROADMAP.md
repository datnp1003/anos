# 🦾 AnosOS Roadmap — GitHub Project

> Paste vào: https://github.com/datnp1003/anos/projects?query=is%3Aopen+add
> Hoặc dùng Issues + Labels thay cho Project

---

## Columns

### ✅ Done
- 10 system tools: Package, Process, Service, FileSystem, Network, User, Cron, Log, SSH, WebServer
- 16 skills with Vietnamese support
- Package auto-detect 6 distros (apt/pacman/dnf/yum/zypper/apk)
- Static musl binary — chạy mọi Linux
- Fix `os error 2` trên sublinux/WSL
- Phase 1: Docker container Alpine 29MB
- Deploy container trên host (port 8788)
- API key auto-load từ OpenClaw config
- anos status / doctor / setup / install-service
- Policy permission system
- ARM64 ISO 35MB bootable
- anos-init: PID 1 + DHCP network + anosd
- CI Release Artifacts (binary arm64 + x86_64)

### 🔄 In Progress
- Policy enforcement for 10 tools (policy.yaml → runtime)
- Docker push ghcr.io (cần token write:packages)

### 📋 Next Up — Phase 2b (Anos Manager)
- Multi-user workers: `anosd --user={user} --socket={user}`
- Per-user memory + AI context riêng biệt
- Per-user policy: admin / operator / guest
- Per-user skills: tùy chỉnh theo role

### 🔮 Future — Phase 3
- Local LLM (llama.cpp + TinyLlama 1.1B)
- Offline mode: cloud → local fallback
- Recovery safe mode: AI fail → shell fallback
- Full CI/CD: auto-build ISO + Docker push
- User UI dashboard (web)
- eBPF system probes
- Voice interface
