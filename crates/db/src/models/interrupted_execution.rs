//! Interrupted execution model for tracking tasks interrupted by server shutdown.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

/// An execution that was interrupted by server shutdown
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct InterruptedExecution {
    pub id: Uuid,
    pub execution_process_id: Uuid,
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    /// JSON serialized ExecutorAction
    pub executor_action: String,
    /// Run reason (e.g., "codingagent")
    pub run_reason: String,
    /// Agent session ID for conversation continuity
    pub agent_session_id: Option<String>,
    /// Executor type (e.g., "ClaudeCode")
    pub executor_type: String,
    pub interrupted_at: DateTime<Utc>,
    pub resumed: bool,
    pub created_at: DateTime<Utc>,
}

/// Data required to create a new interrupted execution record
#[derive(Debug, Clone)]
pub struct CreateInterruptedExecution {
    pub execution_process_id: Uuid,
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub executor_action: String,
    pub run_reason: String,
    pub agent_session_id: Option<String>,
    pub executor_type: String,
}

impl InterruptedExecution {
    /// Create a new interrupted execution record
    pub async fn create(
        pool: &SqlitePool,
        data: &CreateInterruptedExecution,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO interrupted_executions
               (id, execution_process_id, session_id, workspace_id, executor_action, run_reason, agent_session_id, executor_type)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            id,
            data.execution_process_id,
            data.session_id,
            data.workspace_id,
            data.executor_action,
            data.run_reason,
            data.agent_session_id,
            data.executor_type,
        )
        .execute(pool)
        .await?;

        Self::find_by_id(pool, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Find an interrupted execution by ID
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            InterruptedExecution,
            r#"SELECT
                id as "id!: Uuid",
                execution_process_id as "execution_process_id!: Uuid",
                session_id as "session_id!: Uuid",
                workspace_id as "workspace_id!: Uuid",
                executor_action,
                run_reason,
                agent_session_id,
                executor_type,
                interrupted_at as "interrupted_at!: DateTime<Utc>",
                resumed as "resumed!: bool",
                created_at as "created_at!: DateTime<Utc>"
            FROM interrupted_executions WHERE id = ?"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Find all interrupted executions that haven't been resumed yet
    pub async fn find_not_resumed(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            InterruptedExecution,
            r#"SELECT
                id as "id!: Uuid",
                execution_process_id as "execution_process_id!: Uuid",
                session_id as "session_id!: Uuid",
                workspace_id as "workspace_id!: Uuid",
                executor_action,
                run_reason,
                agent_session_id,
                executor_type,
                interrupted_at as "interrupted_at!: DateTime<Utc>",
                resumed as "resumed!: bool",
                created_at as "created_at!: DateTime<Utc>"
            FROM interrupted_executions
            WHERE resumed = 0
            ORDER BY interrupted_at ASC"#,
        )
        .fetch_all(pool)
        .await
    }

    /// Mark an interrupted execution as resumed
    pub async fn mark_resumed(pool: &SqlitePool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE interrupted_executions SET resumed = 1 WHERE id = ?"#,
            id,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Mark all interrupted executions for a session as resumed
    pub async fn mark_resumed_for_session(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            r#"UPDATE interrupted_executions SET resumed = 1 WHERE session_id = ? AND resumed = 0"#,
            session_id,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Clean up old resumed entries
    pub async fn cleanup_old(pool: &SqlitePool, days: i32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            r#"DELETE FROM interrupted_executions
               WHERE resumed = 1
                 AND created_at < datetime('now', '-' || ? || ' days')"#,
            days,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Check if an execution process has an interrupted execution record
    pub async fn find_by_execution_process_id(
        pool: &SqlitePool,
        execution_process_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            InterruptedExecution,
            r#"SELECT
                id as "id!: Uuid",
                execution_process_id as "execution_process_id!: Uuid",
                session_id as "session_id!: Uuid",
                workspace_id as "workspace_id!: Uuid",
                executor_action,
                run_reason,
                agent_session_id,
                executor_type,
                interrupted_at as "interrupted_at!: DateTime<Utc>",
                resumed as "resumed!: bool",
                created_at as "created_at!: DateTime<Utc>"
            FROM interrupted_executions WHERE execution_process_id = ?"#,
            execution_process_id
        )
        .fetch_optional(pool)
        .await
    }
}
