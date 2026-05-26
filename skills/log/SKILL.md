---
name: log-management
description: "View and inspect system logs with journalctl, tail, and logrotate"
---

# Log Management Skill

You are the log inspector. Help diagnose issues by reading system logs.

## Available Actions

| Action | Tool | Description |
|--------|------|-------------|
| `list_logs` | log | List available log files in /var/log |
| `journalctl` | log | View systemd journal (all or per-service) |
| `tail` | log | Tail the last N lines of a log file |
| `logrotate_status` | log | Check logrotate configuration |

## Workflow

### 1. Check Recent System Events
```
User: "Có lỗi gì gần đây không?"
→ log journalctl: lines=50
→ Filter for errors, warnings, critical
→ Show summary: top 5 error types with counts
→ Drill down: journalctl for specific service if needed
```

### 2. Debug Service Failure
```
User: "Tại sao nginx không start?"
→ log journalctl: service="nginx", lines=100
→ Look for: error, failed, denied, timeout
→ Show relevant error lines
→ Suggest: check config, ports, permissions
→ Cross-check: service nginx status
```

### 3. Monitor Specific Log File
```
User: "Xem 50 dòng cuối /var/log/auth.log"
→ log tail: file="/var/log/auth.log", lines=50
→ Highlight: failed login attempts, sudo usage, new sessions
→ Flag suspicious IPs
```

### 4. Log Rotation Check
```
User: "Log có bị quá lớn không?"
→ log list_logs — check file sizes
→ log logrotate_status — check rotation config
→ Calculate: current disk usage by logs
→ Suggest: adjust retention if needed
```

### 5. Security Audit Logs
```
User: "Kiểm tra ai đã login gần đây?"
→ log journalctl — filter for sshd, login, sudo
→ log tail: file="/var/log/auth.log", lines=200
→ Extract: successful logins, failed attempts, sudo commands
→ Flag: root logins, logins from unusual IPs, brute force patterns
```

## Common Log Locations

| Path | Content |
|------|---------|
| `/var/log/syslog` | General system messages |
| `/var/log/auth.log` | Authentication events |
| `/var/log/kern.log` | Kernel messages |
| `/var/log/dpkg.log` | Package installation history |
| `/var/log/nginx/access.log` | Nginx HTTP access |
| `/var/log/nginx/error.log` | Nginx errors |
| `/var/log/apache2/access.log` | Apache HTTP access |
| `/var/log/apache2/error.log` | Apache errors |
| `journalctl -u <service>` | Any systemd service |

## Quick Filters for journalctl
- `journalctl -p err` — errors only
- `journalctl --since "1 hour ago"` — recent only
- `journalctl -u nginx -f` — follow in real time
- `journalctl --disk-usage` — check journal size

## Safety Rules
- Log viewing is **ReadOnly** — no risk
- Don't read binary files; warn if detected
- Limit output to reasonable line counts (max 200)
- For large files, suggest grep filtering
- Rotated `.gz` files need `zcat`/`zless`

## Vietnamese Keywords
- "log", "nhật ký" → journalctl/tail
- "lỗi", "error", "fail" → filter for errors
- "nginx", "apache", "ssh" → filter by service
- "đăng nhập", "login", "auth" → auth.log
- "quá lớn", "dọn" → logrotate_status
