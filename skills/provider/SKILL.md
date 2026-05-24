---
name: provider-management
description: "Configure, switch, and test AI providers — OpenAI-compatible API endpoints"
---

# Provider Management Skill

You are the AI provider manager. Help users configure and switch between AI models.

## Available Commands

| Command | Description |
|---------|-------------|
| `/model` | Show current active provider |
| `/model <id>` | Switch to a different provider |
| `/providers` | List all configured providers |
| `/tools` | List available tools (confirm functionality) |

## Workflow

### 1. "Đổi qua Claude đi"
```
→ /model claude
→ Response: "✅ Switched to Claude (claude/claude-sonnet-4)"
→ Confirm: "Done. Using claude/claude-sonnet-4 now."
```

### 2. "Có những model nào?"
```
→ /providers
→ List all with: ID, Name, Model, Active marker (★)
→ Explain: "Active = currently being used"
```

### 3. "Test model hiện tại"
```
→ "Ping" — sends a simple chat to the model
→ If response OK → "Provider is working ✅"
→ If error → "Provider failed: <error details>. Try /model to switch."
```

## Configuration

Providers are defined in `~/.anos/config/providers.yaml`:

```yaml
active: deepseek

providers:
  - id: deepseek
    name: DeepSeek
    type: openai-compatible
    endpoint: https://api.deepseek.com/v1
    model: deepseek-chat
    api_key_env: ANOS_API_KEY

  - id: ollama
    name: Ollama Local
    type: openai-compatible
    endpoint: http://localhost:11434/v1
    model: llama3
```

### Adding a New Provider

1. Edit `~/.anos/config/providers.yaml`
2. Add a new provider entry with:
   - `id`: short identifier (used in `/model <id>`)
   - `name`: human-readable name
   - `type`: always `openai-compatible`
   - `endpoint`: base URL (automatically appends `/chat/completions`)
   - `model`: the model name to request
   - `api_key_env`: environment variable for API key (optional)
3. Use `/model <id>` to activate

### Compatible APIs
Any OpenAI-compatible HTTP endpoint works:
- Ollama (local)
- vLLM (local)
- OpenRouter
- Groq
- DeepSeek
- Together AI
- Custom 9router proxies

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `401 Unauthorized` | Bad/missing API key | Set `ANOS_API_KEY` env var |
| `Connection refused` | Endpoint down or wrong URL | Check endpoint URL |
| `Provider not found` | Wrong ID in `/model` | Check with `/providers` |
| `Timeout` | Model too slow or network issue | Try a faster provider |

## Safety Rules
- Never expose API keys in responses
- If API key is missing, tell user to set `ANOS_API_KEY`
- Provider switching is instant (ReadOnly). No confirmation needed.

## Vietnamese Keywords
- "model", "đổi model", "chuyển model" → /model
- "provider", "danh sách" → /providers
- "AI nào", "con nào" → show active model
