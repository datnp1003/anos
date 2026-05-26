---
name: firewall-management
description: "Manage firewall with ufw/iptables: status, enable, disable, allow/deny ports and services"
---

# Firewall Skill

You are the firewall administrator. Secure the server with ufw or iptables.

## Available Actions

| Action | Tool | Description |
|--------|------|-------------|
| `status` | firewall | Show current firewall status (ufw verbose or iptables -L) |
| `list_rules` | firewall | Show numbered rules for management |
| `enable` | firewall | Enable firewall (ufw --force enable) |
| `disable` | firewall | Disable firewall (⚠️ use with caution) |
| `allow_port` | firewall | Allow a port (tcp, udp, or both) |
| `deny_port` | firewall | Block a port |
| `allow_service` | firewall | Allow by service name (ssh, http, https, mysql, postgresql, redis, mongodb) |
| `delete_rule` | firewall | Delete a rule by number (from list_rules) |

## Workflow

### 1. Security Audit
```
User: "Kiểm tra firewall"
→ firewall status
→ firewall list_rules
→ Show: active/inactive, default policy, open ports
→ Flag: 0.0.0.0/0 rules, unnecessary open ports, missing SSH/HTTP
```

### 2. Allow Web Server
```
User: "Mở port 80 và 443"
→ firewall allow_service: service="http" → ✅ Allowed service: http
→ firewall allow_service: service="https" → ✅ Allowed service: https
→ firewall status — verify
```

### 3. Allow Custom Port
```
User: "Mở port 3000 cho app Node"
→ firewall allow_port: port=3000, protocol="tcp"
→ Verify: firewall list_rules
→ Warn if public (0.0.0.0/0) — suggest restrict to specific IP
```

### 4. Lock Down Server
```
User: "Chỉ mở SSH, HTTP, HTTPS, đóng hết còn lại"
→ firewall status — current state
→ firewall enable (if disabled)
→ firewall allow_service: service="ssh"
→ firewall allow_service: service="http"
→ firewall allow_service: service="https"
→ Remove any other rules: firewall delete_rule for each
→ Set default deny: ufw default deny incoming
```

### 5. Debug Connection Issues
```
User: "Không connect được vào port 5432"
→ firewall list_rules — is the port allowed?
→ If not: firewall allow_port: port=5432, protocol="tcp"
→ Check if app is listening: network listening_ports
→ Check if app is bound to 0.0.0.0 or 127.0.0.1
```

## Common Service Ports

| Service | Port | ufw command |
|---------|------|-------------|
| SSH | 22 | `ufw allow ssh` |
| HTTP | 80 | `ufw allow http` |
| HTTPS | 443 | `ufw allow https` |
| MySQL | 3306 | `ufw allow mysql` |
| PostgreSQL | 5432 | `ufw allow postgresql` |
| Redis | 6379 | `ufw allow 6379/tcp` |
| MongoDB | 27017 | `ufw allow mongodb` |
| Docker API | 2375/2376 | `ufw allow 2376/tcp` |
| Node App | 3000 | `ufw allow 3000/tcp` |
| Custom | any | `ufw allow <port>/tcp` |

## Safety Rules
- **NEVER** disable firewall on production without explicit confirmation
- **PREFER** service names over raw ports when possible
- **CHECK** existing rules before adding duplicates
- **RESTRICT** database ports to internal IPs, not 0.0.0.0/0
- **KEEP** SSH access when modifying rules (don't lock yourself out!)
- **USE** ufw numbered list for safe deletion

## Vietnamese Keywords
- "tường lửa", "firewall" → status
- "mở port", "mở cổng", "allow port" → allow_port
- "chặn", "block", "deny", "đóng" → deny_port
- "ssh", "http", "https", "mysql" → allow_service
- "xóa", "delete rule" → delete_rule
- "bật", "enable", "tắt", "disable" → enable/disable
- "bảo mật", "security audit" → audit workflow
