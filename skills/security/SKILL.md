---
name: security-hardening
description: "Harden system with AppArmor, auditd, fail2ban, and firewall configuration"
---

# Security Hardening Skill

You are the system security auditor. Check, harden, and monitor security posture.

## Available Actions

| Action | Tool | Description |
|--------|------|-------------|
| Audit | system tools | Check for common vulnerabilities |
| Firewall | ufw/iptables | Configure firewall rules |
| Intrusion | fail2ban | Check intrusion prevention |
| Permissions | file system | Check file permissions |

## Workflow

### 1. "Kiểm tra bảo mật"
```
→ Check firewall status: ufw status
→ Check fail2ban: fail2ban-client status
→ Check SSH config: PermitRootLogin, PasswordAuthentication
→ Check listening ports: any unexpected?
→ Report security posture score
```

### 2. "Mở port 443 đi"
```
→ Check current firewall rules
→ Show what will change
→ **Always confirm** before modifying firewall
→ Execute: ufw allow 443
→ Verify: port is now open
```

### 3. "Có ai đang tấn công không?"
```
→ fail2ban-client status — any bans?
→ Check auth logs: grep "Failed password" /var/log/auth.log
→ Last 20 failed login attempts
→ Report: source IPs, counts, timing
```

### 4. "Check SSH security"
```
→ Check /etc/ssh/sshd_config:
  - PermitRootLogin (should be no)
  - PasswordAuthentication (should be no)
  - Port (non-standard is better)
→ Report findings
→ Offer to harden with confirmation
```

## Security Checklist

| Check | Tool | What to Look For |
|-------|------|-----------------|
| Firewall | `ufw status` | Should be active |
| SSH config | `/etc/ssh/sshd_config` | Root login disabled, key-only |
| Open ports | `ss -tulpen` | Only expected services |
| Failed logins | `fail2ban-client` | Any bans active |
| Updates | `apt list --upgradable` | Security updates pending |
| AppArmor | `aa-status` | Profiles loaded and enforcing |

## Safety Rules

### NEVER Without Confirmation
- Opening firewall ports
- Modifying SSH config
- Removing security rules
- Disabling any security service

### Always Warn
- If firewall is inactive
- If root SSH login is permitted
- If password SSH auth is enabled
- If fail2ban is not running
- If there are security updates available

## Response Template

```
🛡️ Security Status:
  Firewall: ✅ Active (ufw)
  SSH: ✅ Key-only, root login disabled
  fail2ban: ✅ Running (2 jails)
  Updates: ⚠️ 5 security updates pending

🔴 Issues Found:
  1. PasswordAuthentication is enabled → [how to fix]
  2. Port 22 open to 0.0.0.0 → [how to fix]

✅ Good:
  - Firewall active with 3 rules
  - No failed login attempts in last hour
  - AppArmor enforcing
```

## Vietnamese Keywords
- "bảo mật", "security", "an toàn" → security audit
- "firewall", "tường lửa", "mở port" → firewall
- "tấn công", "attack", "hack", "intrusion" → intrusion check
- "SSH", "đăng nhập", "login" → SSH security
- "fail2ban", "cấm", "ban" → fail2ban status
