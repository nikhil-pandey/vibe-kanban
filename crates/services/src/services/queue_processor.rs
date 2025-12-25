//! Queue processor background worker for processing pending task queue entries.

use std::sync::Arc;
use std::time::Duration;

use db::{
    DBService,
    models::{
        execution_process::{ExecutionProcess, ExecutionProcessRunReason},
        session::Session,
        task_queue::QueueEntryStatus,
        workspace::Workspace,
    },
};
use executors::actions::ExecutorAction;
use tokio::{sync::RwLock, task::JoinHandle};

use super::{
    config::{ConcurrencyLimit, Config},
    container::{ContainerError, ContainerService},
    task_queue::{TaskQueueError, TaskQueueService},
};

/// Background worker that processes the task queue
pub struct QueueProcessor {
    db: DBService,
    task_queue: TaskQueueService,
    config: Arc<RwLock<Config>>,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl QueueProcessor {
    /// Start the queue processor as a background task
    /// Returns a handle and a shutdown sender
    pub fn spawn<C: ContainerService + Send + Sync + 'static>(
        db: DBService,
        container: Arc<C>,
        task_queue: TaskQueueService,
        config: Arc<RwLock<Config>>,
    ) -> (JoinHandle<()>, tokio::sync::watch::Sender<bool>) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let processor = QueueProcessor {
            db,
            task_queue,
            config,
            shutdown: shutdown_rx,
        };

        let handle = tokio::spawn(async move {
            processor.run(container).await;
        });

        (handle, shutdown_tx)
    }

    /// Main processing loop
    async fn run<C: ContainerService + Send + Sync + 'static>(mut self, container: Arc<C>) {
        tracing::info!("Queue processor started");

        // Subscribe to queue notifications
        let mut notify_rx = self.task_queue.subscribe();

        loop {
            // Check shutdown signal
            if *self.shutdown.borrow() {
                tracing::info!("Queue processor shutting down");
                break;
            }

            // Try to process any available entries
            match self.try_process_next(container.clone()).await {
                Ok(true) => {
                    // Successfully processed an entry, immediately try another
                    continue;
                }
                Ok(false) => {
                    // No entries to process or no capacity, wait for notification
                }
                Err(e) => {
                    tracing::error!("Queue processor error: {}", e);
                }
            }

            // Wait for notification or timeout (poll every 30 seconds as fallback)
            tokio::select! {
                _ = notify_rx.recv() => {
                    // Got notification, try processing
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    // Periodic check in case notifications were missed
                }
                _ = self.shutdown.changed() => {
                    if *self.shutdown.borrow() {
                        tracing::info!("Queue processor received shutdown signal");
                        break;
                    }
                }
            }
        }

        tracing::info!("Queue processor stopped");
    }

    /// Try to process the next queue entry if capacity is available
    /// Returns true if an entry was processed, false if no entry available or no capacity
    async fn try_process_next<C: ContainerService + Send + Sync + 'static>(
        &self,
        container: Arc<C>,
    ) -> Result<bool, QueueProcessorError> {
        // Check if queue is enabled
        let config = self.config.read().await;
        if !config.concurrency.queue.enabled {
            return Ok(false);
        }

        // Check if we have capacity
        let stats = ExecutionProcess::get_concurrency_stats(&self.db.pool).await?;
        let concurrency_config = &config.concurrency;

        // Check global limit
        if let ConcurrencyLimit::Limited(limit) = concurrency_config.global_limit {
            if stats.total_coding_agents >= limit {
                tracing::debug!(
                    "Queue processor: global limit reached ({}/{})",
                    stats.total_coding_agents,
                    limit
                );
                return Ok(false);
            }
        }

        drop(config); // Release lock before claiming

        // Try to claim an entry that respects per-agent limits
        let entry = self.task_queue.claim_next().await?;
        let entry = match entry {
            Some(e) => e,
            None => return Ok(false),
        };

        tracing::info!(
            "Queue processor: processing entry {} for session {}",
            entry.id,
            entry.session_id
        );

        // Re-check per-agent limit after claiming
        let config = self.config.read().await;
        let concurrency_config = &config.concurrency;

        if let Some(agent_limit) = concurrency_config.agent_limits.get(&entry.executor_type) {
            if let ConcurrencyLimit::Limited(limit) = agent_limit {
                let current = stats.by_executor.get(&entry.executor_type).copied().unwrap_or(0);
                if current >= *limit {
                    tracing::debug!(
                        "Queue processor: agent limit reached for {} ({}/{}), returning to queue",
                        entry.executor_type,
                        current,
                        limit
                    );
                    // Return entry to pending state
                    db::models::task_queue::TaskQueueEntry::update_status(
                        &self.db.pool,
                        entry.id,
                        QueueEntryStatus::Pending,
                        None,
                    )
                    .await?;
                    return Ok(false);
                }
            }
        }

        drop(config);

        // Process the entry
        match self.process_entry(&entry, container).await {
            Ok(()) => {
                self.task_queue.complete(entry.id).await?;
                tracing::info!("Queue processor: completed entry {}", entry.id);
                Ok(true)
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                self.task_queue.fail(entry.id, Some(error_msg.clone())).await?;
                tracing::error!(
                    "Queue processor: failed to process entry {}: {}",
                    entry.id,
                    error_msg
                );
                // Return Ok(true) to continue processing other entries
                Ok(true)
            }
        }
    }

    /// Process a single queue entry by starting its execution
    async fn process_entry<C: ContainerService + Send + Sync + 'static>(
        &self,
        entry: &db::models::task_queue::TaskQueueEntry,
        container: Arc<C>,
    ) -> Result<(), QueueProcessorError> {
        // Load session and workspace
        let session = Session::find_by_id(&self.db.pool, entry.session_id)
            .await?
            .ok_or_else(|| QueueProcessorError::SessionNotFound(entry.session_id))?;

        let workspace = Workspace::find_by_id(&self.db.pool, entry.workspace_id)
            .await?
            .ok_or_else(|| QueueProcessorError::WorkspaceNotFound(entry.workspace_id))?;

        // Deserialize the executor action
        let action: ExecutorAction = serde_json::from_str(&entry.executor_action)
            .map_err(|e| QueueProcessorError::InvalidExecutorAction(e.to_string()))?;

        // Ensure container exists
        container.ensure_container_exists(&workspace).await?;

        // Start execution
        let _execution_process = container
            .start_execution(
                &workspace,
                &session,
                &action,
                &ExecutionProcessRunReason::CodingAgent,
            )
            .await?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QueueProcessorError {
    #[error("Session not found: {0}")]
    SessionNotFound(uuid::Uuid),

    #[error("Workspace not found: {0}")]
    WorkspaceNotFound(uuid::Uuid),

    #[error("Invalid executor action: {0}")]
    InvalidExecutorAction(String),

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    Container(#[from] ContainerError),

    #[error(transparent)]
    TaskQueue(#[from] TaskQueueError),
}
