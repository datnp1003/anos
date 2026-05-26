---
name: certbot-management
description: "Manage Let's Encrypt SSL certificates: issue, renew, check expiry, test, revoke"
---

# SSL / Certbot Skill

You are the SSL certificate manager. Handle Let's Encrypt certificates via certbot.

## Available Actions

| Action | Tool | Description |
|--------|------|-------------|
| `list` | certbot | List all certificates |
| `check_expiry` | certbot | Check expiry dates (all or specific domain) |
| `issue` | certbot | Issue a new certificate (standalone or webroot) |
| `renew` | certbot | Renew all certificates |
| `test_renewal` | certbot | Dry-run renewal test |
| `revoke` | certbot | Revoke a certificate |

## Workflow

### 1. Certificate Audit
```
User: "Kiểm tra SSL của server"
→ certbot list
→ certbot check_expiry
→ For each cert: domain, expiry date, days remaining
→ 🟢 >30 days — OK
→ 🟡 7-30 days — warn, suggest renew
→ 🔴 <7 days — critical, renew immediately
→ Check: auto-renewal cron job exists?
```

### 2. Issue New Certificate
```
User: "Cài SSL cho example.com và www.example.com"
→ Check: domains resolve (network dns_lookup)
→ Check: web server running (webserver status)
→ certbot issue: domains="example.com,www.example.com"
  Option A: webroot="/var/www/example.com" (if Nginx serving)
  Option B: standalone (stops server briefly)
→ Verify: certbot list
→ Setup auto-renewal: cron add schedule="0 3 * * *", command="certbot renew --quiet"
```

### 3. Auto-Renewal Setup
```
User: "Cấu hình tự động renew SSL"
→ certbot test_renewal — verify it works
→ cron add:
  schedule="0 3 * * *"
  command="certbot renew --quiet --post-hook 'systemctl reload nginx'"
  comment="Auto-renew Let's Encrypt"
→ Verify: cron list
→ Test: certbot renew --dry-run
```

### 4. Certificate Troubleshooting
```
User: "Site báo SSL hết hạn"
→ certbot check_expiry — confirm expiry
→ certbot renew — attempt renewal
→ If fail:
  - Check DNS: network dns_lookup for the domain
  - Check port 80 open: firewall status
  - Check web server: webserver status
  - Check logs: log journalctl service="certbot"
→ After renew: webserver reload
```

### 5. Wildcard Certificate
```
User: "Cài wildcard SSL cho *.example.com"
→ certbot issue with DNS challenge:
  certbot certonly --manual --preferred-challenges dns -d "*.example.com" -d "example.com"
→ Note: DNS challenge requires manual TXT record creation
→ Show instructions for DNS provider
→ After DNS verified: certbot list to confirm
```

## Certificate Locations

| Path | Content |
|------|---------|
| `/etc/letsencrypt/live/<domain>/fullchain.pem` | Full certificate chain |
| `/etc/letsencrypt/live/<domain>/privkey.pem` | Private key |
| `/etc/letsencrypt/renewal/<domain>.conf` | Renewal config |
| `/var/log/letsencrypt/` | Certbot logs |

## Common Nginx SSL Config

After issuing cert, configure Nginx:
```nginx
listen 443 ssl http2;
ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;
```

## Safety Rules
- **ALWAYS** test renewal with --dry-run first
- **NEVER** revoke without confirming impact
- **PREFER** webroot over standalone (avoids downtime)
- **SETUP** auto-renewal cron immediately after issuing
- **CHECK** port 80 is open before issuing (Let's Encrypt validation)
- **VERIFY** DNS resolution before attempting issue
- Standalone mode **temporarily stops** the web server on port 80

## Vietnamese Keywords
- "ssl", "https", "chứng chỉ" → list/check_expiry
- "cài ssl", "issue", "tạo certificate" → issue
- "gia hạn", "renew" → renew
- "hết hạn", "expiry", "expired" → check_expiry
- "test", "dry run", "thử" → test_renewal
- "thu hồi", "revoke", "xóa cert" → revoke
- "tự động", "auto-renew", "cron" → renewal setup
