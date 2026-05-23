# 🦾 Anos Agent — Research & Optimal Architecture

> Tổng hợp từ OpenClaw, Claude Code, GitHub Copilot, Aider, Cursor.
> Áp dụng cho AI Native OS — nơi agent **sống trong kernel**, không phải app.

---

## 📊 Comparative Analysis

| Feature | OpenClaw | Claude Code | Copilot | Aider | Cursor | **Anos (Target)** |
|---------|----------|-------------|---------|-------|--------|-------------------|
| **Scope** | General agent | Coding agent | Coding assistant | Code editing | IDE agent | **Full OS** |
| **Language** | TypeScript (Node) | TypeScript | Multi | Python | TypeScript | **Rust** |
| **Context** | Bootstrap files + skills | CLAUDE.md + auto-memory | Project context | **RepoMap (live code graph)** | Codebase index | **SystemMap (live OS graph)** |
| **Memory** | Embedding search | Auto-memory | Limited | Session | Session | **Vector DB + System State History** |
| **Skills** | SKILL.md (markdown) | /commands | Extensions | Commands | Rules | **SKILL.md (extended for OS)** |
| **Sub-agents** | spawn (isolated/fork) | Multi-agent teams | Agent mode | No | Parallel | **OS-level parallel agents** |
| **Tools** | exec/read/write/edit | bash + file edit | IDE tools | Shell + git | IDE tools | **System tools (cgroup, eBPF, kernel, etc.)** |
| **Hooks** | Plugin hooks (lifecycle) | Shell hooks | Events | No | No | **Full hook system (pre/post action)** |
| **Streaming** | Block + delta | SSE | SSE | No | SSE | **gRPC bidirectional stream** |
| **Safety** | Exec approval | Permission gates | Admin controls | Ask | Ask | **4-level permission + audit + snapshot** |
| **Multi-model** | Provider routing | Multi-provider | Model picker | Multi-model | Model picker | **Intent-based auto-routing** |
| **Compaction** | Auto-summarize | Auto-memory | No | No | No | **System-aware compaction** |

---

## 🔑 Key Insight: SystemMap (inspired by Aider's RepoMap)

Aider's breakthrough: **RepoMap** — a graph-ranked, token-optimized map of the codebase.
Anos applies this to the **entire OS**:

```
Aider RepoMap:
  ├── files → symbols (classes, functions, signatures)
  ├── Graph ranking → most referenced = most important
  └── Token budget → dynamic resize

Anos SystemMap:
  ├── Process tree → CPU%, memory, cgroup, systemd units
  ├── Filesystem → Btrfs subvolumes, disk usage, inode counts
  ├── Network → interfaces, firewall rules, connections, latency
  ├── Kernel → sysctl params, loaded modules, eBPF probes
  ├── Services → systemd dependency graph, active/stopped
  ├── Hardware → CPU topology, NUMA, GPU, sensors
  ├── Packages → installed + upgradable, AUR vs official
  ├── Security → AppArmor profiles, audit rules, fail2ban
  └── Graph ranking → most connected components first
```

SystemMap is built **live** on each request, not cached:
- `systemMap.build(tokenBudget)` → returns optimized OS graph
- `systemMap.focus(component)` → deep-dive into one area
- `systemMap.diff(snapshotBefore, snapshotAfter)` → what changed?

---

## 🏗️ Final Anos Agent Architecture

```
                    ┌─────────────────────────────────┐
                    │     User Interface Layer         │
                    │  CLI │ TUI │ Desktop │ Web │ API │
                    └──────────────┬──────────────────┘
                                   │ gRPC / Unix Socket
                    ┌──────────────▼──────────────────┐
                    │        🧠 Anos Agent Core        │
                    │                                  │
                    │  ┌────────────────────────────┐  │
                    │  │  1. Intent Classifier       │  │
                    │  │  NL → {intent, params}      │  │
                    │  └──────────┬─────────────────┘  │
                    │             │                     │
                    │  ┌──────────▼─────────────────┐  │
                    │  │  2. Context Assembler       │  │
                    │  │  SystemMap + Memory + Chat  │  │
                    │  └──────────┬─────────────────┘  │
                    │             │                     │
                    │  ┌──────────▼─────────────────┐  │
                    │  │  3. Provider Router         │  │
                    │  │  Task → best model          │  │
                    │  └──────────┬─────────────────┘  │
                    │             │                     │
                    │  ┌──────────▼─────────────────┐  │
                    │  │  4. Prompt Builder          │  │
                    │  │  SystemPrompt + Skill +     │  │
                    │  │  SystemMap + Context + Task │  │
                    │  └──────────┬─────────────────┘  │
                    │             │                     │
                    │  ┌──────────▼─────────────────┐  │
                    │  │  5. LLM Call (Streaming)    │  │
                    │  └──────────┬─────────────────┘  │
                    │             │                     │
                    │  ┌──────────▼─────────────────┐  │
                    │  │  6. Tool Dispatcher         │  │
                    │  │  Parse tool_call → execute  │  │
                    │  │  → verify → audit log       │  │
                    │  └──────────┬─────────────────┘  │
                    │             │                     │
                    │  ┌──────────▼─────────────────┐  │
                    │  │  7. Response Builder        │  │
                    │  │  Format → stream → persist  │  │
                    │  └────────────────────────────┘  │
                    └──────────────────────────────────┘
```

---

## 📐 Detailed Component Design

### 1. Intent Classifier

```rust
enum Intent {
    Package(PackageAction),   // "cài neovim"
    Diagnostic(DiagTarget),   // "sao máy chậm?"
    Network(NetAction),       // "mở port 443"
    Filesystem(FsAction),     // "dọn disk"
    Process(ProcAction),      // "kill process node"
    Kernel(KernelAction),     // "tune TCP buffer"
    Security(SecAction),      // "check fail2ban"
    SelfUpgrade,              // "nâng cấp OS"
    Gui(GuiAction),           // "cài Hyprland"
    Chat,                     // "chào Mập"
}

fn classify(message: &str) -> (Intent, Confidence) {
    // Fast: keyword match + regex
    // Fallback: ask local LLM (cheap, fast)
}
```

### 2. Context Assembler

Key innovation: **SystemMap** + **Memory** + **Chat History**, token-optimized:

```rust
struct ContextAssembler {
    system_map: SystemMap,
    memory: VectorDb,
    session: ChatHistory,
    token_budget: usize,
}

impl ContextAssembler {
    fn assemble(&self, intent: &Intent) -> AssembledContext {
        // 1. Reserve tokens for response (~20%)
        let available = self.token_budget * 80 / 100;

        // 2. SystemMap → relevant OS state (30% budget)
        let system_context = self.system_map.build(intent, available * 30 / 100);

        // 3. Memory → relevant past fixes/knowledge (20% budget)
        let memory_context = self.memory.search(intent.description(), 5);

        // 4. Chat history → recent messages (50% budget)
        let history = self.session.recent(available * 50 / 100);

        AssembledContext { system_context, memory_context, history }
    }
}
```

### 3. Provider Router

```rust
struct ProviderRouter {
    providers: HashMap<ProviderId, Box<dyn AiProvider>>,
    routing_rules: Vec<RoutingRule>,
}

impl ProviderRouter {
    fn route(&self, intent: &Intent) -> &dyn AiProvider {
        // Rule 1: System action → local LLM (fast, always available, no cost)
        // Rule 2: Code generation → Codex/ACP
        // Rule 3: Complex reasoning → Cloud (Claude/GPT)
        // Rule 4: Simple chat → default
        // Fallback: next priority if provider down
    }
}
```

### 4. Prompt Builder

Builds the final prompt from 4 sources:

```
[SYSTEM PROMPT]          ← anosd identity, rules, safety levels
                          ← ~500 tokens, always present

[SKILL INJECTION]        ← Loaded from skills/<intent>/SKILL.md
                          ← Only the relevant skill

[SYSTEM MAP]             ← Live OS state, token-optimized
                          ← Processes, network, kernel params, etc.

[MEMORY CONTEXT]         ← Relevant past events, fixes, decisions

[CONVERSATION HISTORY]   ← Recent messages in this session

[USER MESSAGE]           ← Current request
```

### 5. Tool Dispatcher — Anos-Specific

```rust
#[async_trait]
trait SystemTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn permission_level(&self) -> PermissionLevel;
    fn parameters(&self) -> JsonSchema;

    async fn execute(&self, params: Value) -> Result<ToolResult>;

    // New: Anos-specific
    fn requires_snapshot(&self) -> bool;  // btrfs snapshot before?
    fn rollback(&self, result: &ToolResult) -> Result<()>;  // undo if needed
}

// Tool Registry
struct ToolRegistry {
    tools: HashMap<String, Box<dyn SystemTool>>,
    audit: AuditLogger,
}

impl ToolRegistry {
    async fn dispatch(&self, call: &ToolCall, level: PermissionLevel) -> Result<ToolResult> {
        let tool = self.tools.get(&call.name)?;

        // Permission gate
        if tool.permission_level() > level {
            return Err("Permission denied. Need higher level.");
        }

        // Snapshot before destructive actions
        if tool.requires_snapshot() {
            btrfs::snapshot("/", &format!("pre-{}-{}", call.name, now()))?;
        }

        // Execute
        self.audit.log(AuditEntry::ActionStart { tool: call.name, params: call.params.clone() });
        let result = tool.execute(call.params.clone()).await;
        self.audit.log(AuditEntry::ActionEnd { tool: call.name, result: result.clone() });

        // Verify
        if result.is_err() && tool.requires_snapshot() {
            // Auto-rollback or ask user
        }

        Ok(result)
    }
}
```

### 6. SystemMap — The Killer Feature

```rust
struct SystemMap {
    components: Vec<SystemComponent>,
    graph: Graph,  // dependency graph for ranking
}

impl SystemMap {
    /// Build a token-optimized map focused on the current intent
    fn build(&self, intent: &Intent, token_budget: usize) -> String {
        let mut relevant = Vec::new();

        match intent {
            Intent::Package(_) => {
                relevant.push(self.package_list());
                relevant.push(self.upgradable_count());
            }
            Intent::Diagnostic(DiagTarget::Performance) => {
                relevant.push(self.top_processes(10));
                relevant.push(self.cpu_info());
                relevant.push(self.memory_info());
                relevant.push(self.io_stats());
            }
            Intent::Diagnostic(DiagTarget::Crash) => {
                relevant.push(self.recent_errors());
                relevant.push(self.oom_events());
                relevant.push(self.core_dumps());
            }
            Intent::Network(_) => {
                relevant.push(self.interfaces());
                relevant.push(self.firewall_rules());
                relevant.push(self.active_connections());
            }
            // ... etc
        }

        // Graph-rank and trim to token budget
        self.rank_and_trim(relevant, token_budget)
    }

    /// Full system snapshot for context
    fn full_snapshot(&self) -> SystemSnapshot {
        SystemSnapshot {
            timestamp: Utc::now(),
            processes: self.process_tree(),
            memory: self.memory_stats(),
            disk: self.disk_stats(),
            network: self.network_stats(),
            kernel: self.kernel_params(),
            services: self.systemd_units(),
            packages: self.package_states(),
        }
    }
}
```

### 7. Memory System

```rust
struct AnosMemory {
    db: QdrantClient,  // Vector DB for semantic search
    states: StateHistory,  // Time-series system snapshots
}

struct MemoryEntry {
    id: Uuid,
    timestamp: DateTime<Utc>,
    category: MemoryCategory,  // Fix, Decision, Lesson, Preference, State
    content: String,
    embedding: Vec<f32>,
    tags: Vec<String>,
}

impl AnosMemory {
    /// Learn from action results
    async fn learn(&self, action: &AuditEntry, result: &ToolResult) {
        if result.is_success() {
            self.remember(MemoryEntry {
                category: MemoryCategory::Fix,
                content: format!("Fixed: {} → {}", action.description(), result.summary()),
                ..
            });
        }
    }

    /// Predict issues from past patterns
    async fn predict(&self, current_state: &SystemSnapshot) -> Vec<Prediction> {
        // Compare current state with past states before failures
        // → "Disk usage growing at current rate → full in 3 days"
    }
}
```

### 8. Safety & Audit System

```rust
#[derive(PartialOrd, PartialEq)]
enum PermissionLevel {
    Suggest = 0,  // Read-only, can only recommend
    Ask = 1,      // Can act after user confirms
    Auto = 2,     // Auto-approved safe actions
    Full = 3,     // Full control, logged + audited
}

struct AuditTrail {
    entries: Vec<AuditEntry>,
}

enum AuditEntry {
    Request { user: String, intent: String, level: PermissionLevel },
    ActionStart { tool: String, params: Value, snapshot: Option<String> },
    ActionResult { tool: String, exit_code: i32, output: String, duration_ms: u64 },
    Verify { tool: String, status: VerifyStatus },
    Rollback { tool: String, reason: String },
}
```

---

## 📊 Architecture Scorecard

| Principle | Implementation | Source |
|-----------|---------------|--------|
| **Live system awareness** | SystemMap — graph-ranked, token-optimized OS graph | Inspired by Aider RepoMap |
| **Extensible skills** | SKILL.md + frontmatter, loaded per-intent | OpenClaw pattern |
| **Multi-model routing** | Intent → best provider, fallback chain | OpenClaw + Claude Code |
| **Safety-first** | 4-level permission + audit + snapshot + rollback | Original design |
| **Context optimization** | SystemMap + Memory + History, token-budgeted | OpenClaw + Aider |
| **Sub-agent parallelism** | Spawn agents for parallel system tasks | OpenClaw + Claude Code |
| **Hook extensibility** | Pre/post action, lifecycle hooks | OpenClaw plugin hooks |
| **Streaming** | gRPC bidirectional stream | Industry standard |
| **Memory** | Vector DB + time-series state history | OpenClaw memory search |
| **Compaction** | System-aware summarization | OpenClaw compaction |
| **Rust-first** | All core in Rust, Python for adapters | Performance + safety |

---

## 🚀 Implementation Order

| Phase | Component | Why This Order |
|-------|-----------|----------------|
| **P0** | Provider trait + OpenAiCompatProvider | Can't do anything without AI |
| **P0** | Unix socket IPC + basic chat flow | Need to talk to user |
| **P1** | SystemMap (process + package) | Core differentiator |
| **P1** | Tool trait + PackageTool + ProcessTool | First system actions |
| **P1** | ContextAssembler + PromptBuilder | End-to-end flow |
| **P2** | IntentClassifier | Smart routing |
| **P2** | Memory (Qdrant) | Learn from past |
| **P2** | AuditLogger + PermissionGate | Safety |
| **P3** | Sub-agent spawn | Parallel work |
| **P3** | Hook system | Extensibility |
| **P4** | Btrfs snapshot integration | Rollback safety |
| **P4** | Self-upgrade tool | Self-evolution |
