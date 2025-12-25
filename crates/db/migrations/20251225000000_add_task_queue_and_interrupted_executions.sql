-- Task queue for storing pending executions when concurrency limit is reached
CREATE TABLE task_queue (
    id              BLOB PRIMARY KEY,
    session_id      BLOB NOT NULL,
    workspace_id    BLOB NOT NULL,
    executor_action TEXT NOT NULL,      -- JSON serialized ExecutorAction
    priority        INTEGER NOT NULL DEFAULT 1000,  -- Lower = higher priority
    status          TEXT NOT NULL DEFAULT 'pending'
                       CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'cancelled')),
    executor_type   TEXT NOT NULL,      -- For per-agent queue tracking
    prompt          TEXT,               -- Original prompt for display
    error_message   TEXT,
    queued_at       TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    started_at      TEXT,
    completed_at    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),

    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

-- Index for efficient queue processing (pending items ordered by priority, then age)
CREATE INDEX idx_task_queue_pending ON task_queue(status, priority, queued_at)
    WHERE status = 'pending';

-- Index for session-based lookups (checking queue position)
CREATE INDEX idx_task_queue_session ON task_queue(session_id, status);

-- Index for workspace-based lookups
CREATE INDEX idx_task_queue_workspace ON task_queue(workspace_id, status);

-- Index for executor type (for per-agent queue depth)
CREATE INDEX idx_task_queue_executor ON task_queue(executor_type, status);

-- Interrupted executions table for tracking tasks that were running when server shut down
CREATE TABLE interrupted_executions (
    id                    BLOB PRIMARY KEY,
    execution_process_id  BLOB NOT NULL UNIQUE,
    session_id            BLOB NOT NULL,
    workspace_id          BLOB NOT NULL,
    executor_action       TEXT NOT NULL,
    run_reason            TEXT NOT NULL,
    agent_session_id      TEXT,           -- For conversation continuity
    executor_type         TEXT NOT NULL,
    interrupted_at        TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    resumed               INTEGER NOT NULL DEFAULT 0,
    created_at            TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),

    FOREIGN KEY (execution_process_id) REFERENCES execution_processes(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

-- Index for finding non-resumed interrupted executions on startup
CREATE INDEX idx_interrupted_executions_not_resumed
    ON interrupted_executions(resumed) WHERE resumed = 0;
