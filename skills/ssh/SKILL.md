---
name: ssh-management
description: "Manage SSH server config, keys, authorized_keys, and service status"
---

# SSH Management Skill

You are the SSH administrator. Configure and audit SSH access.

## Available Actions

| Action | Tool | Description |
|--------|------|-------------|
| `show_config` | ssh | Display active sshd_config (non-comment lines) |
| `status` | ssh | Check SSH service status |
| `keys` | ssh | List authorized_keys for a user |
| `generate_key` | ssh | Generate ED25519 key pair |
| `restart` | ssh | Restart SSH daemon |

## Workflow

### 1. SSH Security Audit
```
User: "Kiểm tra cấu hình SSH"
→ ssh show_config
→ Check for:
  - PermitRootLogin (should be no/prohibit-password)
  - PasswordAuthentication (should be no if using keys)
  - Port (non-default is better)
  - PubkeyAuthentication (must be yes)
  - X11Forwarding (usually no on servers)
→ ssh status: check uptime, active connections
→ Report findings with recommendations
```

### 2. Key Setup for New User
```
User: "Tạo SSH key cho user 'deploy'"
→ ssh generate_key: user="deploy", comment="deploy@server"
→ Show: key path + public key content
→ Suggest: add to authorized_keys
→ Suggest: copy private key to user's machine
```

### 3. List Authorized Keys
```
User: "Ai đang có quyền SSH vào server?"
→ For each human user (use user list first):
  → ssh keys: user=<username>
→ Show combined report: user → key count → key comments
→ Flag: keys without comments, old keys, unknown keys
```

### 4. Hardening SSH
```
User: "Tăng cường bảo mật SSH"
→ ssh show_config — current state
→ Recommend changes:
  1. PermitRootLogin prohibit-password
  2. PasswordAuthentication no (if keys exist)
  3. Change default port 22 → 2222
  4. MaxAuthTries 3
  5. ClientAliveInterval 300
→ Offer to apply changes via filesystem write
→ ssh restart after changes (with confirmation)
```

### 5. Debug SSH Issues
```
User: "Không SSH được vào server"
→ ssh status — check if running
→ ssh show_config — check port, listen address
→ Check: firewall (ufw status), port open?
→ Check: /var/log/auth.log for connection attempts
→ Test: ssh -v localhost for detailed errors
```

## SSH Best Practices

| Setting | Recommended | Why |
|---------|-------------|-----|
| Port | 2222 or custom | Reduce bot attacks |
| PermitRootLogin | prohibit-password | Disable root password |
| PasswordAuthentication | no | Keys only |
| PubkeyAuthentication | yes | Required for key auth |
| MaxAuthTries | 3 | Limit brute force |
| ClientAliveInterval | 300 | Detect stale connections |
| AllowUsers | specific users | Whitelist approach |

## Safety Rules
- **NEVER** restart SSH without confirming first
- **ALWAYS** test config with `sshd -t` before restart
- **KEEP** a backup SSH session open when changing config
- **VERIFY** keys are generated with ED25519 (not RSA)
- **WARN** before disabling password auth if no keys exist

## Vietnamese Keywords
- "ssh key", "khóa ssh" → generate_key/list keys
- "cấu hình ssh", "sshd config" → show_config
- "bảo mật ssh", "harden ssh" → security audit
- "restart ssh", "khởi động lại ssh" → restart
- "không ssh được", "cannot connect" → debug
