//! Task queue service for managing persistent execution queue.

use std::collections::HashMap;
use std::sync::Arc;

use db::{
    DBService,
    models::task_queue::{
        CreateTaskQueueEntry, QueueDepth, QueueEntryStatus, QueuePosition, TaskQueueEntry,
    },
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TaskQueueError {
    #[error("Entry not found: {0}")]
    NotFound(Uuid),

    #[error("Cannot cancel entry in status: {0}")]
    InvalidStatusForCancel(String),

    #[error("Session already has a pending queue entry")]
    AlreadyQueued,

    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Status of the queue for a specific session
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionQueueStatus {
    pub is_queued: bool,
    pub entry: Option<TaskQueueEntry>,
    pub position: Option<QueuePosition>,
}

/// Global queue statistics
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QueueStats {
    pub total_pending: u32,
    pub total_processing: u32,
    #[ts(type = "Record<string, ExecutorQueueStats>")]
    pub by_executor: HashMap<String, ExecutorQueueStats>,
    pub estimated_wait_minutes: Option<u32>,
}

/// Per-executor queue statistics
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExecutorQueueStats {
    pub pending: u32,
    pub processing: u32,
    pub limit: Option<u32>,
}

/// Service for managing the persistent task queue
#[derive(Clone)]
pub struct TaskQueueService {
    db: DBService,
    /// Notification channel for queue processor
    notify_tx: Arc<broadcast::Sender<()>>,
}

impl TaskQueueService {
    pub fn new(db: DBService) -> Self {
        let (notify_tx, _) = broadcast::channel(16);
        Self {
            db,
            notify_tx: Arc::new(notify_tx),
        }
    }

    /// Add a task to the queue
    pub async fn enqueue(
        &self,
        session_id: Uuid,
        workspace_id: Uuid,
        executor_action: String,
        executor_type: String,
        prompt: Option<String>,
        priority: Option<i32>,
    ) -> Result<TaskQueueEntry, TaskQueueError> {
        // Check if session already has a pending entry
        if let Some(_existing) =
            TaskQueueEntry::find_pending_for_session(&self.db.pool, session_id).await?
        {
            return Err(TaskQueueError::AlreadyQueued);
        }

        let entry = TaskQueueEntry::create(
            &self.db.pool,
            &CreateTaskQueueEntry {
                session_id,
                workspace_id,
                executor_action,
                executor_type,
                prompt,
                priority,
            },
        )
        .await?;

        tracing::info!(
            "Task queued: entry_id={}, session_id={}, executor={}",
            entry.id,
            session_id,
            entry.executor_type
        );

        Ok(entry)
    }

    /// Cancel a queued task for a session
    pub async fn cancel_for_session(&self, session_id: Uuid) -> Result<bool, TaskQueueError> {
        let entry = TaskQueueEntry::find_pending_for_session(&self.db.pool, session_id).await?;
        match entry {
            Some(e) => {
                let cancelled = TaskQueueEntry::cancel(&self.db.pool, e.id).await?;
                if cancelled {
                    tracing::info!(
                        "Queue entry cancelled: entry_id={}, session_id={}",
                        e.id,
                        session_id
                    );
                }
                Ok(cancelled)
            }
            None => Ok(false),
        }
    }

    /// Get queue status for a session
    pub async fn get_session_queue_status(
        &self,
        session_id: Uuid,
    ) -> Result<SessionQueueStatus, TaskQueueError> {
        let entry = TaskQueueEntry::find_pending_for_session(&self.db.pool, session_id).await?;
        let position = TaskQueueEntry::get_position(&self.db.pool, session_id).await?;

        Ok(SessionQueueStatus {
            is_queued: entry.is_some(),
            entry,
            position,
        })
    }

    /// Get global queue statistics
    pub async fn get_queue_stats(&self) -> Result<QueueStats, TaskQueueError> {
        let depth = TaskQueueEntry::get_queue_depth(&self.db.pool).await?;

        // Count processing entries
        let processing_count = self.count_processing().await?;

        // Build per-executor stats
        let mut by_executor = HashMap::new();
        for (executor, pending) in depth.by_executor {
            by_executor.insert(
                executor,
                ExecutorQueueStats {
                    pending,
                    processing: 0, // TODO: count processing per executor
                    limit: None,   // Will be filled by caller if needed
                },
            );
        }

        // Estimate wait time: ~5 minutes per task
        let estimated_wait_minutes = if depth.total_pending > 0 {
            Some(depth.total_pending * 5)
        } else {
            None
        };

        Ok(QueueStats {
            total_pending: depth.total_pending,
            total_processing: processing_count,
            by_executor,
            estimated_wait_minutes,
        })
    }

    /// Count processing entries
    async fn count_processing(&self) -> Result<u32, sqlx::Error> {
        TaskQueueEntry::count_by_status(&self.db.pool, QueueEntryStatus::Processing).await
    }

    /// Claim the next pending entry for processing
    pub async fn claim_next(&self) -> Result<Option<TaskQueueEntry>, TaskQueueError> {
        let entry = TaskQueueEntry::claim_next(&self.db.pool).await?;
        if let Some(ref e) = entry {
            tracing::info!(
                "Queue entry claimed: entry_id={}, session_id={}, executor={}",
                e.id,
                e.session_id,
                e.executor_type
            );
        }
        Ok(entry)
    }

    /// Claim the next pending entry for a specific executor type
    pub async fn claim_next_for_executor(
        &self,
        executor_type: &str,
    ) -> Result<Option<TaskQueueEntry>, TaskQueueError> {
        let entry = TaskQueueEntry::claim_next_for_executor(&self.db.pool, executor_type).await?;
        if let Some(ref e) = entry {
            tracing::info!(
                "Queue entry claimed for executor {}: entry_id={}, session_id={}",
                executor_type,
                e.id,
                e.session_id
            );
        }
        Ok(entry)
    }

    /// Mark an entry as completed
    pub async fn complete(&self, entry_id: Uuid) -> Result<(), TaskQueueError> {
        TaskQueueEntry::update_status(&self.db.pool, entry_id, QueueEntryStatus::Completed, None)
            .await?;
        tracing::info!("Queue entry completed: entry_id={}", entry_id);
        Ok(())
    }

    /// Mark an entry as failed
    pub async fn fail(
        &self,
        entry_id: Uuid,
        error_message: Option<String>,
    ) -> Result<(), TaskQueueError> {
        TaskQueueEntry::update_status(
            &self.db.pool,
            entry_id,
            QueueEntryStatus::Failed,
            error_message.clone(),
        )
        .await?;
        tracing::info!(
            "Queue entry failed: entry_id={}, error={:?}",
            entry_id,
            error_message
        );
        Ok(())
    }

    /// Subscribe to queue notifications
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.notify_tx.subscribe()
    }

    /// Notify that capacity may be available (call when an execution completes)
    pub fn notify_capacity_available(&self) {
        // Ignore send errors (no receivers)
        let _ = self.notify_tx.send(());
    }

    /// Get queue depth
    pub async fn get_queue_depth(&self) -> Result<QueueDepth, TaskQueueError> {
        Ok(TaskQueueEntry::get_queue_depth(&self.db.pool).await?)
    }

    /// Get all pending entries (for monitoring)
    pub async fn get_pending_entries(&self) -> Result<Vec<TaskQueueEntry>, TaskQueueError> {
        Ok(TaskQueueEntry::find_all_pending(&self.db.pool).await?)
    }

    /// Reset processing entries to pending (for startup recovery)
    pub async fn reset_orphaned_processing(&self) -> Result<u64, TaskQueueError> {
        let count = TaskQueueEntry::reset_processing_to_pending(&self.db.pool).await?;
        if count > 0 {
            tracing::info!(
                "Reset {} orphaned processing queue entries to pending",
                count
            );
        }
        Ok(count)
    }

    /// Clean up old completed/failed entries
    pub async fn cleanup_old_entries(&self, days: i32) -> Result<u64, TaskQueueError> {
        let count = TaskQueueEntry::cleanup_old(&self.db.pool, days).await?;
        if count > 0 {
            tracing::info!("Cleaned up {} old queue entries", count);
        }
        Ok(count)
    }
}
