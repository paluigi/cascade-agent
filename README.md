# Cascade Agent

**Cascade Agent** is a **robust, asynchronous LLM agentic engine** written in Rust. It is designed to be a reusable CLI tool and library that can be embedded in larger applications. The engine drives a continuous, interruptible agent loop that:

- Sends conversation objects (system, user, memory, tools) to **`llm-cascade`** for inference.
- Parses tool calls from the model, executes them (search, file I/O, dynamic skills, knowledge queries, etc.), and feeds results back into the loop.
- Maintains a token‑aware context, automatically compacting old messages with a summarisation cascade when the token budget is exceeded.
- Persists state on failure and applies exponential back‑off with retry logic.
- Provides a **dynamic skill system** – each skill is a folder with a `SKILL.md` front‑matter and an optional executable script/binary.
- Hosts a **bidirectional orchestrator channel** (WebSocket, with a future gRPC extension) for user interaction, plan approval, and status updates.
- Stores vector embeddings and documents locally using **LanceDB** and **fastembed** (multilingual‑e5‑base).

---

## 🎯 Objectives
1. **Asynchronous, interruptible agent loop** – users can inject updates while the agent is running.
2. **Tool‑first architecture** – every external capability (search, file IO, knowledge) is a first‑class `Tool` implementing a standard trait.
3. **Dynamic skill discovery** – add new capabilities by dropping a folder with `SKILL.md` and an optional script.
4. **Token‑budget management** – keep context within a configurable limit using summarisation.
5. **Local vector store** – embed passages with fastembed, store \& retrieve with LanceDB.
6. **Orchestrator abstraction** – easy swapping between WebSocket and future gRPC.

---

## 🛠️ Usage
### Build & Run
```bash
# Build the binary (debug)
cargo build

# Run the agent with a prompt
cargo run -- run "Create a JSON summary of the latest Rust release notes"
```

### CLI Sub‑commands
| Command | Description |
|--------|-------------|
| `run <prompt>` | Starts an agent session with the given user prompt. |
| `init` | Writes a default `config.toml` (or the `config.example.toml` template). |
| `skills` | Lists all discovered skills from the `data/skills/` directory. |

### Configuration
The agent reads `config.toml` (or a custom path) which contains sections for:
- **Agent** – cascade name, system prompt path (`SOUL.md`).
- **Memory** – token limit, summarisation cascade name, tokenizer model.
- **Knowledge** – LanceDB path, embedding model, collection defaults.
- **Orchestrator** – enable flag, transport (`websocket`), bind address, optional client URL.
- **Search** – API keys for Tavily & Brave, max results.
- **Paths** – directories for skills, plans, outputs, and the vector DB.

---

## 📦 Current Status
- **All core modules implemented** (`agent`, `tools`, `skills`, `memory`, `knowledge`, `orchestrator`, `planning`).
- `cargo check` passes without errors; `cargo clippy` shows no warnings.
- A full **GitHub repository** exists: https://github.com/paluigi-moltis/cascade-agent
- **CLI functional** – you can run a prompt, list skills, and initialise config.
- **Knowledge base** works on machines with glibc ≥ 2.38 (ONNX Runtime binary requirement).
- **Automated tests** for individual modules compile; integration tests need a newer glibc for ONNX Runtime.

---

## ✅ To‑Do List
- [ ] Add **integration tests** for the complete agent loop (mock `llm-cascade` responses).
- [ ] Provide **streaming support** when `llm-cascade` adds a streaming API.
- [ ] Implement **gRPC transport** for the orchestrator (as a plug‑in to the existing trait).
- [ ] Write **documentation** (full API docs, how‑to‑add new skills, debugging tips).
- [ ] Add **CI pipeline** (GitHub Actions) – lint, build, run tests on a container with glibc 2.38+.
- [ ] Publish the crate to **crates.io** (once the public API stabilises).
- [ ] Create a **sample skill** (`example-skill`) that demonstrates the stdin/stdout JSON protocol.
- [ ] Add **demo orchestrator UI** (simple web front‑end that connects via WebSocket).

---

## 📜 License
MIT – feel free to use, modify, and contribute.
