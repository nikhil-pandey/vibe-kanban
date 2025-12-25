//! Task queue model for persistent queue when concurrency limits are reached.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use std::collections::HashMap;
use ts_rs::TS;
use uuid::Uuid;

/// Status of a queue entry
#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, TS)]
#[sqlx(type_name = "queue_entry_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum QueueEntryStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for QueueEntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueEntryStatus::Pending => write!(f, "pending"),
            QueueEntryStatus::Processing => write!(f, "processing"),
            QueueEntryStatus::Completed => write!(f, "completed"),
            QueueEntryStatus::Failed => write!(f, "failed"),
            QueueEntryStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A queue entry representing a pending task execution
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskQueueEntry {
    pub id: Uuid,
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    /// JSON serialized ExecutorAction
    pub executor_action: String,
    /// Priority: lower = higher priority (default 1000)
    pub priority: i32,
    pub status: QueueEntryStatus,
    /// Executor type for per-agent tracking (e.g., "ClaudeCode")
    pub executor_type: String,
    /// Original prompt for display
    pub prompt: Option<String>,
    /// Error message if failed
    pub error_message: Option<String>,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Data required to create a new queue entry
#[derive(Debug, Clone)]
pub struct CreateTaskQueueEntry {
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub executor_action: String,
    pub executor_type: String,
    pub prompt: Option<String>,
    pub priority: Option<i32>,
}

/// Position in the queue for a session
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QueuePosition {
    pub entry_id: Uuid,
    /// 1-based position in the queue
    pub position: u32,
    /// Total entries ahead of this one
    pub total_ahead: u32,
    /// Estimated wait time in minutes (if available)
    pub estimated_wait_minutes: Option<u32>,
}

/// Queue depth statistics
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QueueDepth {
    pub total_pending: u32,
    #[ts(type = "Record<string, number>")]
    pub by_executor: HashMap<String, u32>,
}

/// Status of a session's position in the task queue
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionQueueStatus {
    pub is_queued: bool,
    pub entry: Option<TaskQueueEntry>,
    pub position: Option<QueuePosition>,
}

impl TaskQueueEntry {
    /// Create a new queue entry
    pub async fn create(
        pool: &SqlitePool,
        data: &CreateTaskQueueEntry,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4();
        let priority = data.priority.unwrap_or(1000);
        let status = QueueEntryStatus::Pending.to_string();

        sqlx::query!(
            r#"INSERT INTO task_queue (id, session_id, workspace_id, executor_action, priority, status, executor_type, prompt)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            id,
            data.session_id,
            data.workspace_id,
            data.executor_action,
            priority,
            status,
            data.executor_type,
            data.prompt,
        )
        .execute(pool)
        .await?;

        Self::find_by_id(pool, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Find a queue entry by ID
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            TaskQueueEntry,
            r#"SELECT
                id as "id!: Uuid",
                session_id as "session_id!: Uuid",
                workspace_id as "workspace_id!: Uuid",
                executor_action,
                priority as "priority!: i32",
                status as "status!: QueueEntryStatus",
                executor_type,
                prompt,
                error_message,
                queued_at as "queued_at!: DateTime<Utc>",
                started_at as "started_at?: DateTime<Utc>",
                completed_at as "completed_at?: DateTime<Utc>",
                created_at as "created_at!: DateTime<Utc>",
                updated_at as "updated_at!: DateTime<Utc>"
            FROM task_queue WHERE id = ?"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Find pending entry for a session
    pub async fn find_pending_for_session(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            TaskQueueEntry,
            r#"SELECT
                id as "id!: Uuid",
                session_id as "session_id!: Uuid",
                workspace_id as "workspace_id!: Uuid",
                executor_action,
                priority as "priority!: i32",
                status as "status!: QueueEntryStatus",
                executor_type,
                prompt,
                error_message,
                queued_at as "queued_at!: DateTime<Utc>",
                started_at as "started_at?: DateTime<Utc>",
                completed_at as "completed_at?: DateTime<Utc>",
                created_at as "created_at!: DateTime<Utc>",
                updated_at as "updated_at!: DateTime<Utc>"
            FROM task_queue
            WHERE session_id = ? AND status = 'pending'
            ORDER BY queued_at ASC
            LIMIT 1"#,
            session_id
        )
        .fetch_optional(pool)
        .await
    }

    /// Claim the next pending entry for processing.
    /// Returns None if no entries are available.
    /// Uses a transaction to ensure atomicity.
    pub async fn claim_next(pool: &SqlitePool) -> Result<Option<Self>, sqlx::Error> {
        // Find and update in one query using RETURNING
        let now = Utc::now();
        let pending = QueueEntryStatus::Pending.to_string();
        let processing = QueueEntryStatus::Processing.to_string();

        let result = sqlx::query_as!(
            TaskQueueEntry,
            r#"UPDATE task_queue
               SET status = ?, started_at = ?, updated_at = ?
               WHERE id = (
                   SELECT id FROM task_queue
                   WHERE status = ?
                   ORDER BY priority ASC, queued_at ASC
                   LIMIT 1
               )
               RETURNING
                   id as "id!: Uuid",
                   session_id as "session_id!: Uuid",
                   workspace_id as "workspace_id!: Uuid",
                   executor_action,
                   priority as "priority!: i32",
                   status as "status!: QueueEntryStatus",
                   executor_type,
                   prompt,
                   error_message,
                   queued_at as "queued_at!: DateTime<Utc>",
                   started_at as "started_at?: DateTime<Utc>",
                   completed_at as "completed_at?: DateTime<Utc>",
                   created_at as "created_at!: DateTime<Utc>",
                   updated_at as "updated_at!: DateTime<Utc>""#,
            processing,
            now,
            now,
            pending,
        )
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }

    /// Claim the next pending entry for a specific executor type.
    /// This is used when checking per-agent concurrency limits.
    pub async fn claim_next_for_executor(
        pool: &SqlitePool,
        executor_type: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        let now = Utc::now();
        let pending = QueueEntryStatus::Pending.to_string();
        let processing = QueueEntryStatus::Processing.to_string();

        let result = sqlx::query_as!(
            TaskQueueEntry,
            r#"UPDATE task_queue
               SET status = ?, started_at = ?, updated_at = ?
               WHERE id = (
                   SELECT id FROM task_queue
                   WHERE status = ? AND executor_type = ?
                   ORDER BY priority ASC, queued_at ASC
                   LIMIT 1
               )
               RETURNING
                   id as "id!: Uuid",
                   session_id as "session_id!: Uuid",
                   workspace_id as "workspace_id!: Uuid",
                   executor_action,
                   priority as "priority!: i32",
                   status as "status!: QueueEntryStatus",
                   executor_type,
                   prompt,
                   error_message,
                   queued_at as "queued_at!: DateTime<Utc>",
                   started_at as "started_at?: DateTime<Utc>",
                   completed_at as "completed_at?: DateTime<Utc>",
                   created_at as "created_at!: DateTime<Utc>",
                   updated_at as "updated_at!: DateTime<Utc>""#,
            processing,
            now,
            now,
            pending,
            executor_type,
        )
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }

    /// Update the status of a queue entry
    pub async fn update_status(
        pool: &SqlitePool,
        id: Uuid,
        status: QueueEntryStatus,
        error_message: Option<String>,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        let status_str = status.to_string();
        let completed_at = if matches!(status, QueueEntryStatus::Completed | QueueEntryStatus::Failed | QueueEntryStatus::Cancelled) {
            Some(now)
        } else {
            None
        };

        sqlx::query!(
            r#"UPDATE task_queue
               SET status = ?, error_message = ?, completed_at = ?, updated_at = ?
               WHERE id = ?"#,
            status_str,
            error_message,
            completed_at,
            now,
            id,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Cancel a queue entry
    pub async fn cancel(pool: &SqlitePool, id: Uuid) -> Result<bool, sqlx::Error> {
        let now = Utc::now();
        let cancelled = QueueEntryStatus::Cancelled.to_string();
        let pending = QueueEntryStatus::Pending.to_string();

        let result = sqlx::query!(
            r#"UPDATE task_queue
               SET status = ?, completed_at = ?, updated_at = ?
               WHERE id = ? AND status = ?"#,
            cancelled,
            now,
            now,
            id,
            pending,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Cancel all pending entries for a session
    pub async fn cancel_for_session(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let now = Utc::now();
        let cancelled = QueueEntryStatus::Cancelled.to_string();
        let pending = QueueEntryStatus::Pending.to_string();

        let result = sqlx::query!(
            r#"UPDATE task_queue
               SET status = ?, completed_at = ?, updated_at = ?
               WHERE session_id = ? AND status = ?"#,
            cancelled,
            now,
            now,
            session_id,
            pending,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Cancel all pending entries for a workspace
    pub async fn cancel_for_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let now = Utc::now();
        let cancelled = QueueEntryStatus::Cancelled.to_string();
        let pending = QueueEntryStatus::Pending.to_string();

        let result = sqlx::query!(
            r#"UPDATE task_queue
               SET status = ?, completed_at = ?, updated_at = ?
               WHERE workspace_id = ? AND status = ?"#,
            cancelled,
            now,
            now,
            workspace_id,
            pending,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get the queue position for a session
    pub async fn get_position(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<Option<QueuePosition>, sqlx::Error> {
        // First find the pending entry for this session
        let entry = Self::find_pending_for_session(pool, session_id).await?;
        let entry = match entry {
            Some(e) => e,
            None => return Ok(None),
        };

        // Count how many entries are ahead of this one
        let pending = QueueEntryStatus::Pending.to_string();
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!: i64"
               FROM task_queue
               WHERE status = ?
                 AND (priority < ? OR (priority = ? AND queued_at < ?))"#,
            pending,
            entry.priority,
            entry.priority,
            entry.queued_at,
        )
        .fetch_one(pool)
        .await?;

        let total_ahead = count as u32;
        let position = total_ahead + 1; // 1-based position

        // Estimate wait time: assume ~5 minutes per task ahead
        let estimated_wait_minutes = if total_ahead > 0 {
            Some(total_ahead * 5)
        } else {
            None
        };

        Ok(Some(QueuePosition {
            entry_id: entry.id,
            position,
            total_ahead,
            estimated_wait_minutes,
        }))
    }

    /// Get queue depth statistics
    pub async fn get_queue_depth(pool: &SqlitePool) -> Result<QueueDepth, sqlx::Error> {
        let pending = QueueEntryStatus::Pending.to_string();

        // Get total pending
        let total: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!: i64" FROM task_queue WHERE status = ?"#,
            pending,
        )
        .fetch_one(pool)
        .await?;

        // Get counts by executor
        let rows = sqlx::query!(
            r#"SELECT executor_type, COUNT(*) as "count!: i64"
               FROM task_queue
               WHERE status = ?
               GROUP BY executor_type"#,
            pending,
        )
        .fetch_all(pool)
        .await?;

        let mut by_executor = HashMap::new();
        for row in rows {
            by_executor.insert(row.executor_type, row.count as u32);
        }

        Ok(QueueDepth {
            total_pending: total as u32,
            by_executor,
        })
    }

    /// Get all pending entries (for monitoring/debugging)
    pub async fn find_all_pending(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            TaskQueueEntry,
            r#"SELECT
                id as "id!: Uuid",
                session_id as "session_id!: Uuid",
                workspace_id as "workspace_id!: Uuid",
                executor_action,
                priority as "priority!: i32",
                status as "status!: QueueEntryStatus",
                executor_type,
                prompt,
                error_message,
                queued_at as "queued_at!: DateTime<Utc>",
                started_at as "started_at?: DateTime<Utc>",
                completed_at as "completed_at?: DateTime<Utc>",
                created_at as "created_at!: DateTime<Utc>",
                updated_at as "updated_at!: DateTime<Utc>"
            FROM task_queue
            WHERE status = 'pending'
            ORDER BY priority ASC, queued_at ASC"#,
        )
        .fetch_all(pool)
        .await
    }

    /// Clean up old completed/failed/cancelled entries
    pub async fn cleanup_old(
        pool: &SqlitePool,
        days: i32,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            r#"DELETE FROM task_queue
               WHERE status IN ('completed', 'failed', 'cancelled')
                 AND completed_at < datetime('now', '-' || ? || ' days')"#,
            days,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Reset processing entries back to pending (called on startup for orphaned entries)
    pub async fn reset_processing_to_pending(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let now = Utc::now();
        let pending = QueueEntryStatus::Pending.to_string();
        let processing = QueueEntryStatus::Processing.to_string();

        let result = sqlx::query!(
            r#"UPDATE task_queue
               SET status = ?, started_at = NULL, updated_at = ?
               WHERE status = ?"#,
            pending,
            now,
            processing,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Count entries by status
    pub async fn count_by_status(
        pool: &SqlitePool,
        status: QueueEntryStatus,
    ) -> Result<u32, sqlx::Error> {
        let status_str = status.to_string();
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!: i64" FROM task_queue WHERE status = ?"#,
            status_str,
        )
        .fetch_one(pool)
        .await?;
        Ok(count as u32)
    }
}
