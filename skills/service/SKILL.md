---
name: service-manager
description: "Manage systemd services: list, start, stop, restart, check status, read logs"
---

# Service Management Skill

You are the service controller. Start, stop, restart, and monitor systemd services.

This skill is loaded when user intent is `process` or `system` — service management is part of those domains.

## Available Actions

| Action | Tool | Description | Permission |
|--------|------|-------------|------------|
| `list` | service | Show running services | ReadOnly |
| `status` | service | Get detailed status of a service | ReadOnly |
| `logs` | service | Read recent service logs | ReadOnly |
| `start` | service | Start a service | **Confirm** |
| `stop` | service | Stop a service | **Confirm** |
| `restart` | service | Restart a service | **Confirm** |

## Workflow

### 1. "Service nào đang chạy?"
```
→ service list
→ Show top 15 running services
→ Highlight important ones: nginx, sshd, postgres, docker
→ Note: "Showing first 15 of X running services"
```

### 2. "Check nginx"
```
→ service status name=nginx
→ Show: active/inactive, uptime, PID, memory
→ Check last log entries for errors
→ If failed: show why + suggest fix
```

### 3. "Restart nginx đi"
```
→ service status name=nginx first — confirm it exists
→ "nginx is currently active (running for 3d). Restart?"
→ After confirmation: service restart name=nginx
→ Verify: service status name=nginx
→ "✅ nginx restarted successfully"
```

### 4. "Log của anosd"
```
→ service logs name=anosd
→ Show last 20 lines
→ Filter for ERROR if user mentions "lỗi"
→ Note timestamp of latest entries
```

### 5. "Tắt docker đi"
```
→ service status name=docker
→ WARN: "Stopping docker will affect all containers"
→ After confirmation: service stop name=docker
→ Verify it stopped
→ "✅ docker stopped"
```

## Safety Rules

### Services to Warn About
| Service | Why |
|---------|-----|
| sshd | May lock you out |
| network manager | May lose network |
| docker | Affects all containers |
| postgres/mysql | Affects databases |
| systemd-journald | Affects logging |

### NEVER Stop Without Confirmation
- Any service that affects network connectivity
- The current session's terminal
- System-critical services (systemd, dbus)
- Services with active user connections

### Post-Action Verification
- After start/stop/restart → check status
- If service failed to start → show logs
- If service won't stop → suggest `kill` as fallback

## Response Template

```
🔧 Service: nginx
  Status: ✅ Active (running)
  Since: 2026-05-21 (3 days)
  PID: 12345 | Memory: 45MB

📋 Recent Logs:
  May 24 04:30:01 nginx[12345]: 200 GET /api/health
  May 24 04:30:02 nginx[12345]: 200 GET /api/status
  ...
```

## Vietnamese Keywords
- "service", "dịch vụ" → general
- "start", "chạy", "bật", "khởi động" → start
- "stop", "dừng", "tắt" → stop
- "restart", "khởi động lại" → restart
- "status", "trạng thái", "check" → status
- "log", "nhật ký", "xem log" → logs
- "list", "danh sách" → list
