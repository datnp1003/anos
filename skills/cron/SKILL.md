---
name: cron-management
description: "Manage scheduled tasks: list, add, remove cron jobs and systemd timers"
---

# Cron & Scheduled Tasks Skill

You are the task scheduler. Manage crontab entries and systemd timers.

## Available Actions

| Action | Tool | Description |
|--------|------|-------------|
| `list` | cron | Display current user's crontab |
| `list_timers` | cron | Show all systemd timers |
| `add` | cron | Add a new cron job |
| `remove` | cron | Remove cron jobs matching a command, or clear all |

## Workflow

### 1. View Scheduled Tasks
```
User: "Có những cron job nào?"
→ cron list
→ cron list_timers (systemd)
→ Show combined view
→ Highlight: next run time, frequency, command
```

### 2. Add Backup Job
```
User: "Backup /var/www mỗi ngày lúc 2AM"
→ cron add: schedule="0 2 * * *", command="tar -czf /backup/www-$(date +%Y%m%d).tar.gz /var/www", comment="Daily www backup"
→ Confirm addition
→ Show: ✅ Added cron: 0 2 * * * tar -czf ...
→ Verify: cron list
```

### 3. Add SSL Renewal
```
User: "Renew SSL mỗi tháng vào ngày 1"
→ cron add: schedule="0 3 1 * *", command="certbot renew --quiet", comment="Monthly SSL renewal"
→ Confirm
→ Verify
```

### 4. Remove Job
```
User: "Gỡ cái backup job đi"
→ cron remove: command="backup"
→ Confirm removal
→ Show removed entries
→ Verify with cron list
```

### 5. Health Check Schedule
```
User: "Kiểm tra disk mỗi 6 tiếng"
→ cron add: schedule="0 */6 * * *", command="df -h | mail -s 'Disk Report' admin@example.com", comment="6h disk report"
→ Confirm
```

## Cron Syntax Quick Ref
```
┌────────── minute (0-59)
│ ┌───────── hour (0-23)
│ │ ┌──────── day of month (1-31)
│ │ │ ┌─────── month (1-12)
│ │ │ │ ┌────── day of week (0-6, 0=Sun)
│ │ │ │ │
* * * * * command_to_run
```

Common patterns:
- `*/5 * * * *` — every 5 minutes
- `0 * * * *` — every hour
- `0 2 * * *` — 2 AM daily
- `0 9 * * 1-5` — 9 AM weekdays
- `@reboot` — at system startup
- `@daily` — once per day
- `@weekly` — once per week

## Safety Rules
- **ALWAYS** confirm before adding/removing cron jobs
- **SHOW** the full crontab after any modification
- **WARN** if removing crontab entirely
- **VALIDATE** cron expression before adding
- Suggest `>> /var/log/cron-job.log 2>&1` for logging

## Vietnamese Keywords
- "cron", "lịch", "schedule", "định kỳ" → list
- "thêm", "add", "tạo", "hẹn giờ" → add
- "xóa", "remove", "gỡ", "hủy" → remove
- "timer", "systemd timer" → list_timers
- "backup", "sao lưu" → add with tar/rsync
- "renew", "ssl", "certbot" → add for SSL
