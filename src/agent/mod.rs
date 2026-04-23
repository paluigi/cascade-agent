//! Agent core: the main agentic loop with interrupt handling, tool execution,
//! and memory compaction.

pub mod state;

use std::sync::Arc;

use state::ConversationState;
use tokio::sync::mpsc;

use crate::config::AgentConfig;
use crate::error::{AgentError, Result};
use crate::knowledge::KnowledgeBase;
use crate::memory::MemoryManager;
use crate::orchestrator::types::OrchestratorMessage;
use crate::orchestrator::{create_orchestrator_async, OrchestratorConnection};
use crate::planning::PlanManager;
use crate::skills::SkillManager;
use crate::tools::ToolRegistry;

/// User interrupt injected mid-loop.
#[derive(Debug, Clone)]
pub enum UserInterrupt {
    /// A new user message to inject into the conversation.
    NewMessage(String),
    /// Cancel the current task.
    Cancel,
    /// Edit the current plan.
    EditPlan(String),
}

/// The main agent struct — holds all subsystems and drives the loop.
///
/// `AgentLoop` coordinates between the LLM (via llm-cascade), tool execution,
/// memory compaction, the orchestrator, and the plan manager.
pub struct AgentLoop {
    config: AgentConfig,
    cascade_config: Arc<llm_cascade::AppConfig>,
    db_conn: tokio::sync::Mutex<rusqlite::Connection>,
    state: ConversationState,
    memory: MemoryManager,
    #[allow(dead_code)]
    skill_manager: SkillManager,
    tool_registry: ToolRegistry,
    #[allow(dead_code)]
    knowledge: Option<Arc<KnowledgeBase>>,
    orchestrator: Box<dyn OrchestratorConnection>,
    interrupt_rx: mpsc::Receiver<UserInterrupt>,
    interrupt_tx: mpsc::Sender<UserInterrupt>,
    #[allow(dead_code)]
    plan_manager: Arc<std::sync::Mutex<PlanManager>>,
}

impl AgentLoop {
    /// Create a new agent with all subsystems initialized.
    pub async fn new(config: AgentConfig) -> Result<Self> {
        // Load llm-cascade config
        let cascade_config =
            llm_cascade::load_config(std::path::Path::new(&config.agent.cascade_config_path))
                .map_err(|e| {
                    AgentError::ConfigError(format!("Failed to load cascade config: {}", e))
                })?;

        let cascade_config = Arc::new(cascade_config);

        // Initialize SQLite for llm-cascade
        let db_path = &cascade_config.database.path;
        let cascade_db_conn = llm_cascade::db::init_db(db_path)
            .map_err(|e| AgentError::ConfigError(format!("Failed to init DB: {}", e)))?;

        // Load system prompt (SOUL.md)
        let system_prompt = if std::path::Path::new(&config.agent.soul_md_path).exists() {
            std::fs::read_to_string(&config.agent.soul_md_path)?
        } else {
            "You are Cascade Agent, an autonomous AI assistant.".to_string()
        };

        // Initialize memory (needs its own DB connection)
        let memory_db_conn = llm_cascade::db::init_db(db_path)
            .map_err(|e| AgentError::ConfigError(format!("Failed to init DB for memory: {}", e)))?;
        let memory =
            MemoryManager::new(&config.memory, Arc::clone(&cascade_config), memory_db_conn)?;

        // Initialize skill manager + discover skills
        let mut skill_manager =
            SkillManager::new(std::path::PathBuf::from(&config.paths.skills_dir))?;
        skill_manager.discover()?;

        // Build tool registry
        let tool_registry = ToolRegistry::new();

        // Register built-in tools
        tool_registry.register(crate::tools::builtin::EchoTool);
        tool_registry.register(crate::tools::builtin::ReadFileTool);
        tool_registry.register(crate::tools::builtin::WriteFileTool);
        tool_registry.register(crate::tools::builtin::AskUserTool);

        // Register search tools (optional, only if API keys are available)
        if let Some(tavily) = crate::tools::search::TavilySearchTool::from_env(
            &config.search.tavily_api_key_env,
            config.search.max_results,
        ) {
            tool_registry.register(tavily);
        }
        if let Some(brave) = crate::tools::search::BraveSearchTool::from_env(
            &config.search.brave_api_key_env,
            config.search.max_results,
        ) {
            tool_registry.register(brave);
        }

        // Register skill tools
        for skill_tool in skill_manager.all_tools() {
            tool_registry.register_arc(std::sync::Arc::from(
                skill_tool as Box<dyn crate::tools::Tool>,
            ));
        }

        // Initialize plan manager
        let plan_manager = Arc::new(std::sync::Mutex::new(PlanManager::new(
            std::path::PathBuf::from(&config.paths.plans_dir),
        )?));

        let (interrupt_tx, interrupt_rx) = mpsc::channel::<UserInterrupt>(32);

        let task_id = uuid::Uuid::new_v4().to_string();

        // Register planning tools
        tool_registry.register(crate::tools::planning_tools::CreatePlanTool::new(
            Arc::clone(&plan_manager),
            task_id.clone(),
        ));
        tool_registry.register(crate::tools::planning_tools::UpdatePlanStepTool::new(
            Arc::clone(&plan_manager),
        ));
        tool_registry.register(crate::tools::planning_tools::ListPlansTool::new(
            Arc::clone(&plan_manager),
        ));
        tool_registry.register(crate::tools::planning_tools::GetPlanTool::new(Arc::clone(
            &plan_manager,
        )));

        // Register list_tools last (needs registry snapshot)
        tool_registry.register_list_tools();

        // Initialize knowledge base (may fail if ONNX Runtime is unavailable)
        let knowledge = match KnowledgeBase::new(&config.knowledge).await {
            Ok(kb) => {
                tracing::info!(target: "agent", "Knowledge base initialized (collection: {})", config.knowledge.default_collection);
                Some(Arc::new(kb))
            }
            Err(e) => {
                tracing::warn!(target: "agent", "Knowledge base init failed, running without it: {}", e);
                None
            }
        };

        // Register knowledge query tool
        if let Some(ref kb) = knowledge {
            let kq_tool = crate::tools::knowledge_tool::KnowledgeQueryTool::with_defaults(
                Arc::clone(kb) as Arc<dyn crate::tools::knowledge_tool::KnowledgeProvider>,
                config.knowledge.default_collection.clone(),
                config.knowledge.max_results,
            );
            tool_registry.register(kq_tool);
        }

        // Initialize orchestrator
        let orchestrator = create_orchestrator_async(&config.orchestrator).await?;

        let state = ConversationState::new(system_prompt, task_id);

        Ok(Self {
            config,
            cascade_config,
            db_conn: tokio::sync::Mutex::new(cascade_db_conn),
            state,
            memory,
            skill_manager,
            tool_registry,
            knowledge,
            orchestrator,
            interrupt_rx,
            interrupt_tx,
            plan_manager,
        })
    }

    /// Get a handle to send interrupts into the running loop.
    pub fn interrupt_sender(&self) -> mpsc::Sender<UserInterrupt> {
        self.interrupt_tx.clone()
    }

    /// Run the agent loop until completion, error, or cancellation.
    ///
    /// Returns the final assistant text output.
    pub async fn run(&mut self, initial_prompt: String) -> Result<String> {
        self.state.add_user_message(initial_prompt.clone());
        self.orchestrator
            .push(OrchestratorMessage::TaskStarted {
                task_id: self.state.task_id.clone(),
                description: initial_prompt,
            })
            .await;

        let result = self.run_loop().await;

        match &result {
            Ok(_output) => {
                self.orchestrator
                    .push(OrchestratorMessage::TaskCompleted {
                        task_id: self.state.task_id.clone(),
                        output_path: None,
                    })
                    .await;
            }
            Err(e) => {
                self.orchestrator
                    .push(OrchestratorMessage::Error(e.to_string()))
                    .await;
            }
        }

        result
    }

    /// Internal agentic loop: LLM call → tool execution → repeat.
    async fn run_loop(&mut self) -> Result<String> {
        loop {
            // 1. Check for pending user interrupts (non-blocking)
            while let Ok(interrupt) = self.interrupt_rx.try_recv() {
                match interrupt {
                    UserInterrupt::NewMessage(msg) => {
                        self.state.add_user_message(msg);
                    }
                    UserInterrupt::Cancel => {
                        self.orchestrator
                            .push(OrchestratorMessage::TaskCancelled)
                            .await;
                        return Ok("Task cancelled by user.".into());
                    }
                    UserInterrupt::EditPlan(content) => {
                        self.state
                            .add_user_message(format!("[Plan Edit]: {}", content));
                    }
                }
            }

            // 2. Check memory budget
            let token_count = self.memory.count_tokens(&self.state);
            if self.memory.should_compact(token_count) {
                match self.memory.compact(&mut self.state).await {
                    Ok(report) => {
                        self.orchestrator
                            .push(OrchestratorMessage::ContextCompacted {
                                before: report.tokens_before,
                                after: report.tokens_after,
                            })
                            .await;
                        tracing::info!(
                            target: "agent",
                            "Context compacted: {} -> {} tokens ({} msgs -> {} msgs)",
                            report.tokens_before,
                            report.tokens_after,
                            report.messages_before,
                            report.messages_after
                        );
                    }
                    Err(e) => {
                        tracing::warn!(target: "agent", "Compaction failed: {}", e);
                    }
                }
            }

            // 3. Build conversation with tool definitions
            let tool_defs = self.tool_registry.tool_definitions();
            let conversation = self.state.to_conversation().with_tools(tool_defs);

            // 4. Send to llm-cascade, while also listening for orchestrator messages
            let cascade_name = self.config.agent.cascade_name.clone();
            let config = Arc::clone(&self.cascade_config);

            let cascade_future = async {
                let conn_lock = self.db_conn.lock().await;
                llm_cascade::run_cascade(&cascade_name, &conversation, &config, &conn_lock).await
            };

            let response = tokio::select! {
                cascade_result = cascade_future => {
                    cascade_result
                }
                Some(orch_msg) = self.orchestrator.recv() => {
                    self.handle_orchestrator_message(orch_msg).await;
                    continue;
                }
            };

            let response = match response {
                Ok(r) => r,
                Err(cascade_err) => {
                    let saved_path = self.state.to_json_file(&self.config.paths.outputs_dir)?;
                    tracing::error!(
                        target: "agent",
                        "All cascade providers failed: {}. State saved to: {:?}",
                        cascade_err.message,
                        saved_path
                    );
                    return Err(AgentError::InferenceFailed(cascade_err.message));
                }
            };

            // 5. Process response content blocks
            let mut has_tool_calls = false;

            for block in &response.content {
                match block {
                    llm_cascade::ContentBlock::Text { text } => {
                        if !text.is_empty() {
                            self.state.add_assistant_text(text.clone());
                            self.orchestrator
                                .push(OrchestratorMessage::AssistantText(text.clone()))
                                .await;
                        }
                    }
                    llm_cascade::ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                    } => {
                        has_tool_calls = true;
                        let start = std::time::Instant::now();

                        let args: serde_json::Value = serde_json::from_str(arguments)
                            .unwrap_or(serde_json::json!({"raw": arguments}));

                        tracing::info!(
                            target: "agent",
                            "Executing tool: {} (id={})",
                            name,
                            id
                        );

                        let tool_result = if name == "ask_user" {
                            self.handle_ask_user(&args).await
                        } else {
                            self.tool_registry.execute(name, args.clone()).await
                        };
                        let duration_ms = start.elapsed().as_millis() as u64;

                        let result_str = match &tool_result {
                            Ok(r) => r.to_json_string(),
                            Err(e) => serde_json::json!({
                                "status": "error",
                                "error": e.to_string()
                            })
                            .to_string(),
                        };

                        self.state.add_tool_result(id, &result_str);

                        let status_str = match &tool_result {
                            Ok(r) => r.status_str(),
                            Err(_) => "error",
                        };

                        self.orchestrator
                            .push(OrchestratorMessage::ToolExecuted {
                                tool: name.clone(),
                                status: status_str.to_string(),
                                duration_ms,
                            })
                            .await;

                        // Auto-store search results in knowledge base
                        if (name == "tavily_search" || name == "brave_search")
                            && tool_result.is_ok()
                        {
                            self.store_search_results(name, &args, &tool_result).await;
                        }
                    }
                }
            }

            // 6. Check termination conditions
            if !has_tool_calls {
                break;
            }
            if self.state.turn_count >= self.config.agent.max_tool_rounds {
                self.orchestrator
                    .push(OrchestratorMessage::Warning(format!(
                        "Max tool rounds ({}) reached",
                        self.config.agent.max_tool_rounds
                    )))
                    .await;
                break;
            }
        }

        Ok(self.state.last_assistant_text().unwrap_or_default())
    }

    /// Handle an inbound message from the orchestrator.
    async fn handle_orchestrator_message(&mut self, msg: OrchestratorMessage) {
        match msg {
            OrchestratorMessage::UserReply { content } => {
                self.state.add_user_message(content);
            }
            OrchestratorMessage::PlanApproval { approved, feedback } => {
                let feedback_text = feedback.unwrap_or_default();
                let approval_msg = if approved {
                    format!("[Plan Approved] {}", feedback_text)
                } else {
                    format!("[Plan Rejected] {}", feedback_text)
                };
                self.state.add_user_message(approval_msg);
            }
            OrchestratorMessage::CancelTask => {
                self.orchestrator
                    .push(OrchestratorMessage::TaskCancelled)
                    .await;
            }
            _ => {
                tracing::debug!(target: "agent", "Ignoring orchestrator message: {:?}", msg);
            }
        }
    }

    /// Intercept ask_user tool calls: push question to orchestrator, await reply.
    async fn handle_ask_user(
        &mut self,
        args: &serde_json::Value,
    ) -> crate::error::Result<crate::tools::ToolResult> {
        let question = args
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("(no question provided)");

        if !self.orchestrator.is_connected() {
            return Ok(crate::tools::ToolResult::ok(serde_json::json!({
                "status": "no_orchestrator",
                "question": question,
                "answer": "No orchestrator connected. Use the interrupt channel to reply.",
            })));
        }

        self.orchestrator
            .push(OrchestratorMessage::UserQuestion {
                question: question.to_string(),
            })
            .await;

        tracing::info!(target: "agent", "Waiting for user reply to: {}", question);

        match tokio::time::timeout(
            std::time::Duration::from_secs(300),
            self.orchestrator.recv(),
        )
        .await
        {
            Ok(Some(OrchestratorMessage::UserReply { content })) => {
                Ok(crate::tools::ToolResult::ok(serde_json::json!({
                    "status": "replied",
                    "question": question,
                    "answer": content,
                })))
            }
            Ok(Some(other)) => {
                self.handle_orchestrator_message(other).await;
                Ok(crate::tools::ToolResult::ok(serde_json::json!({
                    "status": "interrupted",
                    "question": question,
                    "answer": "User sent a different message instead of replying.",
                })))
            }
            Ok(None) => Ok(crate::tools::ToolResult::ok(serde_json::json!({
                "status": "timeout",
                "question": question,
                "answer": "Orchestrator disconnected before user replied.",
            }))),
            Err(_) => Ok(crate::tools::ToolResult::ok(serde_json::json!({
                "status": "timeout",
                "question": question,
                "answer": "Timed out waiting for user reply (5 minutes).",
            }))),
        }
    }

    /// Store search results in the knowledge base for future retrieval.
    async fn store_search_results(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
        tool_result: &crate::error::Result<crate::tools::ToolResult>,
    ) {
        let kb = match &self.knowledge {
            Some(kb) => kb,
            None => return,
        };

        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return,
        };

        let results = match tool_result {
            Ok(r) => &r.data,
            Err(_) => return,
        };

        let result_array = match results.get("results").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return,
        };

        let entries: Vec<crate::knowledge::vectordb::KnowledgeEntry> = result_array
            .iter()
            .filter_map(|item| {
                let text = format!(
                    "{}\n{}",
                    item.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    item.get("snippet").and_then(|v| v.as_str()).unwrap_or("")
                );
                if text.trim().is_empty() {
                    return None;
                }
                Some(crate::knowledge::vectordb::KnowledgeEntry {
                    text,
                    source: tool_name.to_string(),
                    metadata: serde_json::json!({
                        "url": item.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                        "query": query,
                    }),
                    timestamp: chrono::Utc::now().timestamp(),
                })
            })
            .collect();

        if entries.is_empty() {
            return;
        }

        let collection = &self.config.knowledge.default_collection;
        match kb.store_results(collection, entries).await {
            Ok(()) => {
                tracing::info!(
                    target: "agent",
                    "Stored {} search results in knowledge base (collection: {})",
                    result_array.len(),
                    collection
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "agent",
                    "Failed to store search results in knowledge base: {}",
                    e
                );
            }
        }
    }

    /// Update the system prompt dynamically.
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.state.system_prompt = prompt;
    }

    /// Get a reference to the current conversation state.
    pub fn state(&self) -> &ConversationState {
        &self.state
    }
}
