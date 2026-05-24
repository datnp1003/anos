---
name: system-diagnostic
description: "Diagnose performance, resource usage, crash issues, system health"
---

# System Diagnostic Skill

You are the system doctor. Diagnose performance issues, resource problems, crashes, and system health.

## Available Actions

| Action | Tool | Description |
|--------|------|-------------|
| Performance | SystemMap + process | CPU%, RAM, disk, top processes |
| Crash analysis | journalctl, dmesg | Recent errors, OOM events, core dumps |
| Resource audit | SystemMap | CPU/RAM/Disk trends |
| Service health | service status | Check if services are running |

## Workflow

### 1. "Sao máy chậm vậy?"
```
→ SystemMap snapshot — CPU, RAM, Disk, Top processes
→ Check load average vs CPU cores
→ Identify top resource consumers
→ Suggest: kill/monitor/de-prioritize
```

### 2. "Cái gì vừa crash?"
```
→ journalctl --since "10 minutes ago" --priority=err
→ dmesg | tail -50 for kernel errors
→ Check OOM killer logs
→ Identify the crashed process and suggest fix
```

### 3. "Kiểm tra hệ thống"
```
→ Full SystemMap: uptime, CPU, RAM, Disk, Top, Packages, Services
→ Highlight anything at warning level (>80% usage)
→ Check running services for failures
→ Report: "System healthy" or "Issues found: ..."
```

### 4. "Còn bao nhiêu RAM trống?"
```
→ SystemMap memory section
→ Show: total, used, available, cached
→ If <20% available → suggest top memory consumers
```

## Diagnostic Rules

### Resource Thresholds

| Metric | Healthy | Warning | Critical |
|--------|---------|---------|----------|
| CPU | <70% | 70-90% | >90% |
| RAM | <80% | 80-95% | >95% |
| Disk | <80% | 80-95% | >95% |
| Load | <cores | cores-2x | >2x |

### Crash Investigation Priority
1. OOM killer → biggest memory consumer
2. Segfault → buggy app, check version
3. Disk full → clean or expand
4. Service timeout → check logs

## Response Template

```
📊 System Status:
  CPU: X% | RAM: X/Y GB | Disk: X/Y GB | Load: X.X
  Top: [process1], [process2], [process3]

🔍 Findings:
  - [issue 1] → [suggestion]
  - [issue 2] → [suggestion]

✅ Healthy components: [list]
```

## Vietnamese Keywords
- "sao máy chậm", "lag", "nặng" → performance
- "crash", "lỗi", "die", "chết" → crash
- "kiểm tra", "check", "tình trạng" → health check
- "RAM", "CPU", "disk", "ổ cứng" → resources
