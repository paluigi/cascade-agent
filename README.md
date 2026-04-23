# Cascade Agent

**Cascade Agent** is a **robust, asynchronous LLM agentic engine** written in Rust. It is designed to be a reusable CLI tool and library that can be embedded in larger applications. The engine drives a continuous, interruptible agent loop that:

- Sends conversation objects (system, user, memory, tools) to **`llm-cascade`** for inference.
- Parses tool calls from the model, executes them (search, file I/O, dynamic skills, knowledge queries, planning, etc.), and feeds results back into the loop.
- Maintains a token‑aware context, automatically compacting old messages with a summarisation cascade when the token budget is exceeded.
- Persists state on failure and applies exponential back‑off with retry logic.
- Provides a **dynamic skill system** – each skill is a folder with a `SKILL.md` front‑matter and an optional executable script/binary.
- Hosts a **bidirectional orchestrator channel** (WebSocket, with a future gRPC extension) for user interaction, plan approval, and status updates.
- Stores vector embeddings and documents locally using **LanceDB** and **fastembed** (multilingual‑e5‑base).
- Automatically stores web search results in the knowledge base for future retrieval.

---

## Objectives
1. **Asynchronous, interruptible agent loop** – users can inject updates while the agent is running.
2. **Tool‑first architecture** – every external capability (search, file IO, knowledge, planning) is a first‑class `Tool` implementing a standard trait.
3. **Dynamic skill discovery** – add new capabilities by dropping a folder with `SKILL.md` and an optional script.
4. **Token‑budget management** – keep context within a configurable limit using summarisation.
5. **Local vector store** – embed passages with fastembed, store & retrieve with LanceDB.
6. **Orchestrator abstraction** – easy swapping between WebSocket and future gRPC.
7. **Planning system** – create, update, and track execution plans stored as TOML with a markdown rendering view.

---

## Usage
### Build & Run
```bash
cargo build
cargo run -- run "Create a JSON summary of the latest Rust release notes"
```

### CLI Sub‑commands
| Command | Description |
|--------|-------------|
| `run <prompt>` | Starts an agent session with the given user prompt. |
| `init` | Writes a default `config.toml`. |
| `skills` | Lists all discovered skills from the `data/skills/` directory. |
| `plans` | Lists all plans with their status and progress. |
| `show-plan <path>` | Displays a specific plan file. |

### Configuration
The agent reads `config.toml` (or a custom path via `--config`) which contains sections for:
- **Agent** – cascade name, system prompt path (`SOUL.md`).
- **Memory** – token limit, summarisation cascade name, tokenizer model.
- **Knowledge** – LanceDB path, embedding model, collection defaults.
- **Orchestrator** – enable flag, transport (`websocket`), bind address, optional client URL.
- **Search** – API keys for Tavily & Brave, max results.
- **Paths** – directories for skills, plans, outputs, and the vector DB.

### Adding a New Skill
1. Create a directory under `data/skills/` (e.g., `data/skills/my-skill/`).
2. Add a `SKILL.md` file with YAML frontmatter and instructions body.
3. Optionally add an executable (`run.sh`, `run.py`, or a binary).
4. The agent discovers skills at startup and registers them as tools.

See `data/skills/example-skill/` for a reference implementation.

---

## Current Status
- **All core modules implemented and wired** (`agent`, `tools`, `skills`, `memory`, `knowledge`, `orchestrator`, `planning`).
- `cargo check`, `cargo clippy`, and `cargo test` all pass cleanly (22 tests: 13 unit + 9 integration).
- **Knowledge base** wired into agent loop; search results auto-stored.
- **Planning tools** (create_plan, update_plan_step, list_plans, get_plan) registered and available to the LLM.
- **Orchestrator** supports both client mode (connect to WS server) and server mode (host WS server for orchestrator UI).
- **ask_user** tool intercepts calls and routes through the orchestrator with a 5-minute timeout.
- **Bidirectional orchestrator** – agent listens for `UserReply`, `PlanApproval`, and `CancelTask` messages via `tokio::select!`.
- **TOML storage** – plans stored as `.toml` files (lossless serde round-trip); markdown is a read-only `render_markdown()` view.
- Knowledge base works on machines with glibc >= 2.38 (ONNX Runtime binary requirement).

---

## To‑Do List
- [ ] Provide **streaming support** when `llm-cascade` adds a streaming API.
- [ ] Implement **gRPC transport** for the orchestrator (as a plug‑in to the existing trait).
- [ ] Write **documentation** (full API docs, how‑to‑add new skills, debugging tips).
- [ ] Add **CI pipeline** (GitHub Actions) – lint, build, run tests on a container with glibc 2.38+.
- [ ] Publish the crate to **crates.io** (once the public API stabilises).
- [ ] Add **demo orchestrator UI** (simple web front‑end that connects via WebSocket).

---

## Known Limitations

- **DB mutex held during LLM inference** – the `db_conn` lock is held for the entire cascade call duration. Currently only one DB user exists so there's no contention, but this should be restructured to release the lock before returning results if additional DB consumers are added.
- **`list_plans` loads all plans from disk** – every plan file is read and deserialized. Acceptable at current scale (plans are small, typically < 1KB) but would benefit from pagination or caching if plan count grows significantly.

---

## Change Log
- **2026-04-23**: Migrated plan storage from markdown to TOML (serde-native, lossless round-trip). Markdown is now a read-only `render_markdown()` view. Removed `parse_markdown()` and helper functions. Updated CLI `show-plan` to load via PlanManager and render. Added known limitations section to README.

---

## License
MIT – feel free to use, modify, and contribute.
