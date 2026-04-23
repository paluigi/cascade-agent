pub mod server;
pub mod types;
pub mod websocket;

use crate::config::OrchestratorSettings;
use crate::error::Result;
use async_trait::async_trait;
use tokio::sync::broadcast;
use types::OrchestratorMessage;

/// Trait for bidirectional orchestrator communication.
#[async_trait]
pub trait OrchestratorConnection: Send + Sync {
    /// Push a message from agent to orchestrator (fire-and-forget)
    async fn push(&self, message: OrchestratorMessage);

    /// Receive the next message from orchestrator (blocking await)
    async fn recv(&mut self) -> Option<OrchestratorMessage>;

    /// Check if connected
    fn is_connected(&self) -> bool;
}

/// No-op implementation for when orchestrator is disabled.
pub struct NoopOrchestrator;

#[async_trait]
impl OrchestratorConnection for NoopOrchestrator {
    async fn push(&self, message: OrchestratorMessage) {
        tracing::debug!(target: "orchestrator", "Noop push: {:?}", message);
    }
    async fn recv(&mut self) -> Option<OrchestratorMessage> {
        std::future::pending().await
    }
    fn is_connected(&self) -> bool {
        false
    }
}

/// Server-mode orchestrator: the agent hosts a WebSocket server and communicates
/// via broadcast (outbound) and mpsc (inbound) channels.
pub struct ServerOrchestrator {
    broadcast_tx: broadcast::Sender<OrchestratorMessage>,
    inbound_rx: tokio::sync::mpsc::Receiver<OrchestratorMessage>,
    #[allow(dead_code)]
    server_handle: Option<tokio::task::JoinHandle<crate::error::Result<()>>>,
}

impl std::fmt::Debug for ServerOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerOrchestrator").finish()
    }
}

impl ServerOrchestrator {
    pub async fn new(bind_address: &str) -> Result<Self> {
        let (server, broadcast_tx, inbound_rx) = server::OrchestratorServer::new(bind_address);
        let handle = tokio::spawn(server.run());
        Ok(Self {
            broadcast_tx,
            inbound_rx,
            server_handle: Some(handle),
        })
    }
}

#[async_trait]
impl OrchestratorConnection for ServerOrchestrator {
    async fn push(&self, message: OrchestratorMessage) {
        let _ = self.broadcast_tx.send(message);
    }

    async fn recv(&mut self) -> Option<OrchestratorMessage> {
        self.inbound_rx.recv().await
    }

    fn is_connected(&self) -> bool {
        self.broadcast_tx.receiver_count() > 0
    }
}

/// Factory function to create the right orchestrator transport.
pub fn create_orchestrator(
    config: &OrchestratorSettings,
) -> Result<Box<dyn OrchestratorConnection>> {
    if !config.enabled {
        return Ok(Box::new(NoopOrchestrator));
    }
    match config.transport.as_str() {
        "websocket" => Ok(Box::new(NoopOrchestrator)),
        other => Err(crate::error::AgentError::OrchestratorError(format!(
            "Unknown transport: {}",
            other
        ))),
    }
}

/// Async factory for transports that need initialization (like WebSocket).
pub async fn create_orchestrator_async(
    config: &OrchestratorSettings,
) -> Result<Box<dyn OrchestratorConnection>> {
    if !config.enabled {
        return Ok(Box::new(NoopOrchestrator));
    }
    match config.transport.as_str() {
        "websocket" => {
            if let Some(url) = &config.connect_url {
                let ws = websocket::WebSocketOrchestrator::connect(url).await?;
                Ok(Box::new(ws))
            } else {
                let server = ServerOrchestrator::new(&config.bind_address).await?;
                Ok(Box::new(server))
            }
        }
        other => Err(crate::error::AgentError::OrchestratorError(format!(
            "Unknown transport: {}",
            other
        ))),
    }
}
