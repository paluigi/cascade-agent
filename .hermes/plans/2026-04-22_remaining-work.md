# Cascade Agent — Remaining Work Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Status:** All core modules are coded and the project compiles cleanly (`cargo check` + `cargo clippy` pass). The main gaps are **integration wiring**, **2 failing tests**, and **missing data/scaffolding**.

---

## Current State Summary

| Module | Code Status | Wired into Agent Loop? |
|--------|------------|----------------------|
| `error.rs` | Complete | Yes |
| `config.rs` | Complete | Yes |
| `agent/` (loop + state) | Complete | Yes |
| `tools/` (trait + registry + builtins) | Complete | Yes |
| `tools/search.rs` | Complete | Yes |
| `tools/knowledge_tool.rs` | Complete | **No** — `KnowledgeBase` not instantiated in `AgentLoop::new()` |
| `skills/` | Complete | Yes (discovery at startup) |
| `memory/` | Complete | Yes |
| `knowledge/` (vectordb + embeddings) | Complete | **No** — not instantiated |
| `orchestrator/` | Complete | Partially — push works, recv() never called, server never started |
| `planning/` | Complete | **No** — `PlanManager` initialized but never used |

---

## Phase A: Fix Existing Issues (Quick Wins)

### A1. Fix 2 Failing Tokenizer Tests
- `test_estimate_tokens` — assertion expects 2, got 1 (fallback heuristic `len/4` of a short string)
- `test_count_messages_with_roles` — assertion expects 22, got 20 (role overhead estimates changed)
- **Action:** Update test assertions to match actual tokenizer output, or adjust the hardcoded overhead constants in `count_messages()`.

### A2. Fix Planning Markdown Round-Trip Data Loss
- `parse_markdown()` always regenerates UUID, clears `task_id`, resets status to `Draft`, loses step statuses.
- **Action:** Embed `id`, `task_id`, `status`, and per-step statuses in the markdown rendering (e.g., as HTML comments or a YAML metadata block at the top). Update `parse_markdown()` to recover them.

### A3. Create `data/` Directory Structure + `.gitkeep`
- `data/skills/`, `data/plans/`, `data/outputs/`, `data/lancedb/`
- Add `.gitkeep` files so the structure is tracked in git.
- Create `data/skills/example-skill/SKILL.md` + `data/skills/example-skill/run.sh` as a reference skill demonstrating the stdin/stdout JSON protocol.

---

## Phase B: Wire Knowledge Base into Agent Loop

### B1. Instantiate `KnowledgeBase` in `AgentLoop::new()`
- Create `KnowledgeBase::new()` using config from `config.knowledge`.
- Wrap in `Arc` for thread safety.
- Register `KnowledgeQueryTool` with the `ToolRegistry`, passing the `KnowledgeBase` as the `KnowledgeProvider`.

### B2. Auto-Store Search Results in Knowledge Base
- After `TavilySearchTool` and `BraveSearchTool` execute successfully, store results in the knowledge base.
- **Option A:** Add a post-execution hook in the agent loop (after any tool returns, if it's a search tool, store results).
- **Option B:** Create a `SearchAndStoreTool` wrapper that delegates to the search tool then stores.
- **Recommendation:** Option A — add a `ToolResultInterceptor` or post-processing step in `run_loop()`.

---

## Phase C: Wire Planning into Agent Loop

### C1. Add Planning Built-in Tools
Create tools in `tools/builtin.rs`:
- `CreatePlanTool` — takes `title` + `steps: Vec<String>`, calls `PlanManager::create_plan()`.
- `UpdatePlanStepTool` — takes `plan_id`, `step_number`, `status`, optional `result`, calls `PlanManager::update_step()`.
- `ListPlansTool` — lists all plans with their statuses.
- `GetPlanTool` — loads and renders a specific plan as markdown.

### C2. Register Planning Tools in Agent Loop
- Add the 4 planning tools to the `ToolRegistry` in `AgentLoop::new()`.
- Remove `#[allow(dead_code)]` from `plan_manager`.

### C3. Auto-Plan Integration (Optional Enhancement)
- Add logic in `run_loop()` to detect when the LLM creates a plan (via `CreatePlanTool`), and set the conversation's plan context.
- Push `OrchestratorMessage::StepUpdate` when plan steps are marked complete.

---

## Phase D: Complete Orchestrator Integration

### D1. Listen for Inbound Orchestrator Messages
- In `run_loop()`, use `tokio::select!` to simultaneously:
  1. Process the current turn (LLM call + tool execution).
  2. Listen on `orchestrator.recv()` for incoming messages.
- Handle incoming `UserReply`, `PlanApproval`, and `CancelTask` messages.

### D2. Start Orchestrator Server When Enabled
- In `AgentLoop::new()` or `run()`, if `config.orchestrator.enabled` and no `connect_url`, start the `OrchestratorServer` in a background tokio task.
- The agent hosts the server; an external orchestrator UI connects as a client.

### D3. Wire `AskUserTool` Interception
- When the LLM calls `ask_user`, instead of executing the tool normally:
  1. Push `OrchestratorMessage::UserQuestion` to the orchestrator.
  2. Await a `UserReply` from the orchestrator (with timeout).
  3. Return the user's reply as the tool result.
- Fallback: if no orchestrator is connected, return a message asking the user to use the interrupt channel.

---

## Phase E: Integration Tests

### E1. Agent Loop Integration Test (Mock LLM)
- Create a mock/stub for `llm_cascade::run_cascade` (may require feature flags or a test harness).
- Test: send a prompt → mock LLM returns a tool call → tool executes → mock LLM returns text → loop exits.
- Test: interrupt with cancel → loop exits gracefully.
- Test: memory compaction triggers when token limit exceeded.

### E2. Skill Execution Integration Test
- Create a temp skill with a `run.sh` that echoes JSON to stdout.
- Discover the skill, register as tool, execute via `ToolRegistry`.

### E3. Knowledge Base Integration Test
- Insert entries into a temp LanceDB, query, verify relevance.
- **Note:** Requires glibc ≥ 2.38 for ONNX Runtime. Mark test with `#[cfg_attr(not(target_env = "musl"), ignore)]` or run only in CI.

---

## Phase F: Polish & Documentation

### F1. CLI Enhancements
- Add `--config` flag to `run` subcommand (currently config path is hardcoded to `config.toml`).
- Add `plans` subcommand to list/manage plans from CLI.
- Add `knowledge` subcommand to query the knowledge base directly.

### F2. Example Skill
- Create `data/skills/example-skill/` with:
  - `SKILL.md` — frontmatter with name, description, input schema.
  - `run.sh` — reads JSON from stdin, processes it, writes JSON to stdout.

### F3. SOUL.md Enhancement
- Update `SOUL.md` to mention available tools, planning capabilities, and knowledge base.

---

## Priority Order

| Priority | Phase | Effort | Impact |
|----------|-------|--------|--------|
| **P0** | A1 (Fix failing tests) | Small | CI green |
| **P0** | A3 (data/ structure + example skill) | Small | Developer experience |
| **P1** | B1 (Wire knowledge base) | Medium | Core feature unlocked |
| **P1** | C1+C2 (Planning tools) | Medium | Core feature unlocked |
| **P1** | D1 (Orchestrator recv) | Medium | Bidirectional communication |
| **P2** | A2 (Planning markdown fix) | Small | Data integrity |
| **P2** | B2 (Auto-store search results) | Small | Knowledge accumulation |
| **P2** | D2+D3 (Server + AskUser) | Medium | Full orchestrator UX |
| **P3** | E1-E3 (Integration tests) | Large | Reliability |
| **P3** | F1-F3 (CLI + docs) | Medium | Usability |

---

## Verification Commands

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```
