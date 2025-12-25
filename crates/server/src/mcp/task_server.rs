use std::{future::Future, str::FromStr};

use chrono::{DateTime, Utc};
use db::models::{
    project::{CreateProject, Project, UpdateProject},
    project_repo::CreateProjectRepo,
    repo::Repo,
    tag::Tag,
    task::{CreateTask, Task, TaskStatus, TaskWithAttemptStatus, UpdateTask},
    workspace::{Workspace, WorkspaceContext},
};
use executors::{executors::BaseCodingAgent, profile::ExecutorProfileId};
use regex::Regex;
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::tool::{Parameters, ToolRouter},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json;
use uuid::Uuid;

use crate::routes::{
    containers::ContainerQuery,
    task_attempts::{CreateTaskAttemptBody, TaskAttemptDiffResponse, WorkspaceRepoInput},
};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateTaskInput {
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Optional description of the task")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateTasksRequest {
    #[schemars(description = "The ID of the project to create the task(s) in. This is required!")]
    pub project_id: Uuid,
    #[schemars(description = "One or more tasks to create")]
    pub tasks: Vec<CreateTaskInput>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreatedTaskSummary {
    #[schemars(description = "The ID of the created task")]
    pub task_id: String,
    #[schemars(description = "The title of the created task")]
    pub title: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreateTasksResponse {
    #[schemars(description = "The tasks that were created")]
    pub tasks: Vec<CreatedTaskSummary>,
    #[schemars(description = "How many tasks were created")]
    pub count: usize,
    #[schemars(description = "Any tasks that failed to create")]
    pub failed: Vec<BatchOperationError>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProjectSummary {
    #[schemars(description = "The unique identifier of the project")]
    pub id: String,
    #[schemars(description = "The name of the project")]
    pub name: String,
    #[schemars(description = "When the project was created")]
    pub created_at: String,
    #[schemars(description = "When the project was last updated")]
    pub updated_at: String,
}

impl ProjectSummary {
    fn from_project(project: Project) -> Self {
        Self {
            id: project.id.to_string(),
            name: project.name,
            created_at: project.created_at.to_rfc3339(),
            updated_at: project.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateProjectRepoRequest {
    #[schemars(description = "Display name for the repository inside the project")]
    pub display_name: String,
    #[schemars(description = "Absolute path to the local git repository")]
    pub git_repo_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateProjectRequest {
    #[schemars(description = "Name of the project")]
    pub name: String,
    #[schemars(description = "One or more repositories to link to the project")]
    pub repositories: Vec<CreateProjectRepoRequest>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateProjectsRequest {
    #[schemars(description = "One or more projects to create")]
    pub projects: Vec<CreateProjectRequest>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreateProjectResponse {
    #[schemars(description = "Summary of the created project")]
    pub project: ProjectSummary,
    #[schemars(description = "How many repositories were linked to the project")]
    pub repository_count: usize,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreateProjectsResponse {
    #[schemars(description = "Summaries of the created projects")]
    pub projects: Vec<CreateProjectResponse>,
    #[schemars(description = "How many projects were created")]
    pub count: usize,
    #[schemars(description = "Any projects that failed to create")]
    pub failed: Vec<BatchOperationError>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpRepoSummary {
    #[schemars(description = "The unique identifier of the repository")]
    pub id: String,
    #[schemars(description = "The name of the repository")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListReposRequest {
    #[schemars(description = "The ID of the project to list repositories from")]
    pub project_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListReposResponse {
    pub repos: Vec<McpRepoSummary>,
    pub count: usize,
    pub project_id: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListProjectsResponse {
    pub projects: Vec<ProjectSummary>,
    pub count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListTasksRequest {
    #[schemars(description = "The ID of the project to list tasks from")]
    pub project_id: Uuid,
    #[schemars(
        description = "Optional status filter: 'todo', 'inprogress', 'inreview', 'done', 'cancelled'"
    )]
    pub status: Option<String>,
    #[schemars(description = "Maximum number of tasks to return (default: 50)")]
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TaskSummary {
    #[schemars(description = "The unique identifier of the task")]
    pub id: String,
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Current status of the task")]
    pub status: String,
    #[schemars(description = "When the task was created")]
    pub created_at: String,
    #[schemars(description = "When the task was last updated")]
    pub updated_at: String,
    #[schemars(description = "Whether the task has an in-progress execution attempt")]
    pub has_in_progress_attempt: Option<bool>,
    #[schemars(description = "Whether the last execution attempt failed")]
    pub last_attempt_failed: Option<bool>,
}

impl TaskSummary {
    fn from_task_with_status(task: TaskWithAttemptStatus) -> Self {
        Self {
            id: task.id.to_string(),
            title: task.title.to_string(),
            status: task.status.to_string(),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
            has_in_progress_attempt: Some(task.has_in_progress_attempt),
            last_attempt_failed: Some(task.last_attempt_failed),
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TaskDetails {
    #[schemars(description = "The unique identifier of the task")]
    pub id: String,
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Optional description of the task")]
    pub description: Option<String>,
    #[schemars(description = "Current status of the task")]
    pub status: String,
    #[schemars(description = "When the task was created")]
    pub created_at: String,
    #[schemars(description = "When the task was last updated")]
    pub updated_at: String,
    #[schemars(description = "Whether the task has an in-progress execution attempt")]
    pub has_in_progress_attempt: Option<bool>,
    #[schemars(description = "Whether the last execution attempt failed")]
    pub last_attempt_failed: Option<bool>,
}

impl TaskDetails {
    fn from_task(task: Task) -> Self {
        Self {
            id: task.id.to_string(),
            title: task.title,
            description: task.description,
            status: task.status.to_string(),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
            has_in_progress_attempt: None,
            last_attempt_failed: None,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListTasksResponse {
    pub tasks: Vec<TaskSummary>,
    pub count: usize,
    pub project_id: String,
    pub applied_filters: ListTasksFilters,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListTasksFilters {
    pub status: Option<String>,
    pub limit: i32,
}

#[derive(Debug, Serialize, schemars::JsonSchema, Deserialize)]
pub struct ListTasksByStatusRequest {
    #[schemars(description = "The ID of the project to list tasks from")]
    pub project_id: Uuid,
    #[schemars(description = "Maximum number of tasks to return (default: 200)")]
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize, schemars::JsonSchema, Deserialize)]
pub struct TaskWithMergeSummary {
    #[schemars(description = "The unique identifier of the task")]
    pub id: String,
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Current status of the task")]
    pub status: String,
    #[schemars(description = "When the task was last updated")]
    pub updated_at: String,
    #[schemars(description = "Whether work for the task has been merged")]
    pub is_merged: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema, Deserialize)]
pub struct TasksByStatusGroup {
    #[schemars(description = "Status bucket name")]
    pub status: String,
    #[schemars(description = "Tasks in this status")]
    pub tasks: Vec<TaskWithMergeSummary>,
}

#[derive(Debug, Serialize, schemars::JsonSchema, Deserialize)]
pub struct ListTasksByStatusResponse {
    #[schemars(description = "The project these tasks belong to")]
    pub project_id: String,
    #[schemars(description = "Tasks grouped by status with merge info")]
    pub groups: Vec<TasksByStatusGroup>,
    #[schemars(description = "Total number of tasks across all groups")]
    pub count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateTaskInput {
    #[schemars(description = "The ID of the task to update")]
    pub task_id: Uuid,
    #[schemars(description = "New title for the task")]
    pub title: Option<String>,
    #[schemars(description = "New description for the task")]
    pub description: Option<String>,
    #[schemars(description = "New status: 'todo', 'inprogress', 'inreview', 'done', 'cancelled'")]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateTasksRequest {
    #[schemars(description = "One or more task updates to apply")]
    pub tasks: Vec<UpdateTaskInput>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct UpdateTasksResponse {
    #[schemars(description = "Updated tasks")]
    pub tasks: Vec<TaskDetails>,
    #[schemars(description = "How many tasks were updated")]
    pub count: usize,
    #[schemars(description = "Any tasks that failed to update")]
    pub failed: Vec<BatchOperationError>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteTasksRequest {
    #[schemars(description = "The IDs of the tasks to delete")]
    pub task_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct McpWorkspaceRepoInput {
    #[schemars(description = "The repository ID")]
    pub repo_id: Uuid,
    #[schemars(description = "The base branch for this repository")]
    pub base_branch: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StartWorkspaceSessionRequest {
    #[schemars(description = "The ID of the task to start")]
    pub task_id: Uuid,
    #[schemars(
        description = "The coding agent executor to run ('CLAUDE_CODE', 'CODEX', 'GEMINI', 'CURSOR_AGENT', 'OPENCODE')"
    )]
    pub executor: String,
    #[schemars(description = "Optional executor variant, if needed")]
    pub variant: Option<String>,
    #[schemars(description = "Base branch for each repository in the project")]
    pub repos: Vec<McpWorkspaceRepoInput>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StartWorkspaceSessionResponse {
    pub task_id: String,
    pub workspace_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StartWorkspaceSessionsRequest {
    #[schemars(description = "One or more task attempts to start")]
    pub sessions: Vec<StartWorkspaceSessionRequest>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StartWorkspaceSessionsResponse {
    #[schemars(description = "Started workspace sessions")]
    pub sessions: Vec<StartWorkspaceSessionResponse>,
    #[schemars(description = "How many sessions were started")]
    pub count: usize,
    #[schemars(description = "Any task attempts that failed to start")]
    pub failed: Vec<BatchOperationError>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DeleteTasksResponse {
    #[schemars(description = "The IDs of deleted tasks")]
    pub deleted_task_ids: Vec<String>,
    #[schemars(description = "How many tasks were deleted")]
    pub count: usize,
    #[schemars(description = "Any tasks that failed to delete")]
    pub failed: Vec<BatchOperationError>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateProjectInput {
    #[schemars(description = "The ID of the project to update")]
    pub project_id: Uuid,
    #[schemars(description = "New project name")]
    pub name: Option<String>,
    #[schemars(description = "Optional dev script command")]
    pub dev_script: Option<String>,
    #[schemars(description = "Optional dev script working directory")]
    pub dev_script_working_dir: Option<String>,
    #[schemars(description = "Optional default agent working directory")]
    pub default_agent_working_dir: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateProjectsRequest {
    #[schemars(description = "One or more project updates to apply")]
    pub projects: Vec<UpdateProjectInput>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct UpdateProjectsResponse {
    #[schemars(description = "Updated project summaries")]
    pub projects: Vec<ProjectSummary>,
    #[schemars(description = "How many projects were updated")]
    pub count: usize,
    #[schemars(description = "Any projects that failed to update")]
    pub failed: Vec<BatchOperationError>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteProjectsRequest {
    #[schemars(description = "The IDs of the projects to delete")]
    pub project_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DeleteProjectsResponse {
    #[schemars(description = "The IDs of deleted projects")]
    pub deleted_project_ids: Vec<String>,
    #[schemars(description = "How many projects were deleted")]
    pub count: usize,
    #[schemars(description = "Any projects that failed to delete")]
    pub failed: Vec<BatchOperationError>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTasksRequest {
    #[schemars(description = "The IDs of the tasks to retrieve")]
    pub task_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptDiffRequest {
    #[schemars(description = "Optional attempt/workspace ID to fetch the diff for")]
    #[serde(alias = "attemptId")]
    pub attempt_id: Option<Uuid>,
    #[schemars(
        description = "Set to true to fetch the newest attempt (uses the current task context when available)"
    )]
    #[serde(default)]
    pub latest: Option<bool>,
    #[schemars(description = "Include aggregated stats (additions/deletions) in the response")]
    #[serde(default, alias = "includeStats")]
    pub include_stats: Option<bool>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetTasksResponse {
    #[schemars(description = "Task details for each requested task")]
    pub tasks: Vec<TaskDetails>,
    #[schemars(description = "How many tasks were returned")]
    pub count: usize,
    #[schemars(description = "Any tasks that failed to fetch")]
    pub failed: Vec<BatchOperationError>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BatchOperationError {
    #[schemars(description = "Identifier for the item that failed (id or index)")]
    pub identifier: String,
    #[schemars(description = "Error message for the failure")]
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct TaskServer {
    client: reqwest::Client,
    base_url: String,
    tool_router: ToolRouter<TaskServer>,
    context: Option<McpContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct McpRepoContext {
    #[schemars(description = "The unique identifier of the repository")]
    pub repo_id: Uuid,
    #[schemars(description = "The name of the repository")]
    pub repo_name: String,
    #[schemars(description = "The target branch for this repository in this workspace")]
    pub target_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct McpContext {
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub workspace_id: Uuid,
    pub workspace_branch: String,
    #[schemars(
        description = "Repository info and target branches for each repo in this workspace"
    )]
    pub workspace_repos: Vec<McpRepoContext>,
}

impl TaskServer {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
            tool_router: Self::tool_router(),
            context: None,
        }
    }

    pub async fn init(mut self) -> Self {
        let context = self.fetch_context_at_startup().await;

        if context.is_none() {
            self.tool_router.map.remove("get_context");
            tracing::debug!("VK context not available, get_context tool will not be registered");
        } else {
            tracing::info!("VK context loaded, get_context tool available");
        }

        self.context = context;
        self
    }

    async fn fetch_context_at_startup(&self) -> Option<McpContext> {
        let current_dir = std::env::current_dir().ok()?;
        let canonical_path = current_dir.canonicalize().unwrap_or(current_dir);
        let normalized_path = utils::path::normalize_macos_private_alias(&canonical_path);

        let url = self.url("/api/containers/attempt-context");
        let query = ContainerQuery {
            container_ref: normalized_path.to_string_lossy().to_string(),
        };

        let response = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            self.client.get(&url).query(&query).send(),
        )
        .await
        .ok()?
        .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let api_response: ApiResponseEnvelope<WorkspaceContext> = response.json().await.ok()?;

        if !api_response.success {
            return None;
        }

        let ctx = api_response.data?;

        // Map RepoWithTargetBranch to McpRepoContext
        let workspace_repos: Vec<McpRepoContext> = ctx
            .workspace_repos
            .into_iter()
            .map(|rwb| McpRepoContext {
                repo_id: rwb.repo.id,
                repo_name: rwb.repo.name,
                target_branch: rwb.target_branch,
            })
            .collect();

        Some(McpContext {
            project_id: ctx.project.id,
            task_id: ctx.task.id,
            task_title: ctx.task.title,
            workspace_id: ctx.workspace.id,
            workspace_branch: ctx.workspace.branch,
            workspace_repos,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponseEnvelope<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiTaskWithMerge {
    id: Uuid,
    title: String,
    status: TaskStatus,
    updated_at: DateTime<Utc>,
    is_merged: bool,
}

#[derive(Debug, Deserialize)]
struct ApiTasksByStatusGroup {
    status: TaskStatus,
    tasks: Vec<ApiTaskWithMerge>,
}

impl TaskServer {
    fn success<T: Serialize>(data: &T) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(data)
                .unwrap_or_else(|_| "Failed to serialize response".to_string()),
        )]))
    }

    fn err_value(v: serde_json::Value) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::error(vec![Content::text(
            serde_json::to_string_pretty(&v)
                .unwrap_or_else(|_| "Failed to serialize error".to_string()),
        )]))
    }

    fn err<S: Into<String>>(msg: S, details: Option<S>) -> Result<CallToolResult, ErrorData> {
        let mut v = serde_json::json!({"success": false, "error": msg.into()});
        if let Some(d) = details {
            v["details"] = serde_json::json!(d.into());
        };
        Self::err_value(v)
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        rb: reqwest::RequestBuilder,
    ) -> Result<T, CallToolResult> {
        let resp = rb
            .send()
            .await
            .map_err(|e| Self::err("Failed to connect to VK API", Some(&e.to_string())).unwrap())?;

        let status = resp.status();
        let body_bytes = resp.bytes().await.map_err(|e| {
            Self::err("Failed to read VK API response", Some(&e.to_string())).unwrap()
        })?;
        let body = String::from_utf8_lossy(&body_bytes);

        tracing::debug!(status = %status, body = %body, "VK API raw response");

        if !status.is_success() {
            return Err(Self::err(
                format!("VK API returned error status: {}", status),
                Some(body.to_string()),
            )
            .unwrap());
        }

        let api_response =
            serde_json::from_slice::<ApiResponseEnvelope<T>>(&body_bytes).map_err(|e| {
                tracing::warn!(
                    status = %status,
                    body = %body,
                    error = %e,
                    "Failed to parse VK API response"
                );
                Self::err("Failed to parse VK API response", Some(&e.to_string())).unwrap()
            })?;

        if !api_response.success {
            let msg = api_response.message.as_deref().unwrap_or("Unknown error");
            return Err(Self::err("VK API returned error", Some(msg)).unwrap());
        }

        match api_response.data {
            Some(data) => Ok(data),
            // Some VK endpoints (e.g. 202 task deletion) return `success: true` without a
            // `data` payload. Accept these by treating a missing payload as JSON null and
            // attempting to deserialize to the requested type.
            None => serde_json::from_value(serde_json::Value::Null)
                .map_err(|_| Self::err("VK API response missing data field", None).unwrap()),
        }
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Expands @tagname references in text by replacing them with tag content.
    /// Returns the original text if expansion fails (e.g., network error).
    /// Unknown tags are left as-is (not expanded, not an error).
    async fn expand_tags(&self, text: &str) -> String {
        // Pattern matches @tagname where tagname is non-whitespace, non-@ characters
        let tag_pattern = match Regex::new(r"@([^\s@]+)") {
            Ok(re) => re,
            Err(_) => return text.to_string(),
        };

        // Find all unique tag names referenced in the text
        let tag_names: Vec<String> = tag_pattern
            .captures_iter(text)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if tag_names.is_empty() {
            return text.to_string();
        }

        // Fetch all tags from the API
        let url = self.url("/api/tags");
        let tags: Vec<Tag> = match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<ApiResponseEnvelope<Vec<Tag>>>().await {
                    Ok(envelope) if envelope.success => envelope.data.unwrap_or_default(),
                    _ => return text.to_string(),
                }
            }
            _ => return text.to_string(),
        };

        // Build a map of tag_name -> content for quick lookup
        let tag_map: std::collections::HashMap<&str, &str> = tags
            .iter()
            .map(|t| (t.tag_name.as_str(), t.content.as_str()))
            .collect();

        // Replace each @tagname with its content (if found)
        let result = tag_pattern.replace_all(text, |caps: &regex::Captures| {
            let tag_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            match tag_map.get(tag_name) {
                Some(content) => (*content).to_string(),
                None => caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string(),
            }
        });

        result.into_owned()
    }

    fn summarize_error(err: CallToolResult) -> String {
        if let Some(structured) = err.structured_content {
            return structured.to_string();
        }

        let text_parts: Vec<String> = err
            .content
            .unwrap_or_default()
            .into_iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect();

        if text_parts.is_empty() {
            "Unknown error".to_string()
        } else {
            text_parts.join("\n")
        }
    }

    async fn resolve_attempt_id(
        &self,
        attempt_id: Option<Uuid>,
        latest: bool,
    ) -> Result<Uuid, CallToolResult> {
        if let Some(id) = attempt_id {
            return Ok(id);
        }

        if !latest {
            return Err(Self::err(
                "attempt_id is required unless latest=true",
                Some("Pass attempt_id or set latest to true to use the newest attempt"),
            )
            .unwrap());
        }

        let mut url = self.url("/api/task-attempts");
        if let Some(ctx) = &self.context {
            url.push_str(&format!("?task_id={}", ctx.task_id));
        }

        let attempts: Vec<Workspace> = match self.send_json(self.client.get(&url)).await {
            Ok(list) => list,
            Err(err) => return Err(err),
        };

        attempts.first().map(|ws| ws.id).ok_or_else(|| {
            Self::err(
                "No task attempts found",
                Some("Start a workspace session or provide an explicit attempt_id"),
            )
            .unwrap()
        })
    }
}

#[tool_router]
impl TaskServer {
    #[tool(
        description = "Return project, task, and workspace metadata for the current workspace session context."
    )]
    async fn get_context(&self) -> Result<CallToolResult, ErrorData> {
        // Context was fetched at startup and cached
        // This tool is only registered if context exists, so unwrap is safe
        let context = self.context.as_ref().expect("VK context should exist");
        TaskServer::success(context)
    }

    #[tool(
        description = "Create one or many tasks/tickets in a project. Always pass the `project_id` and an array of tasks."
    )]
    async fn create_tasks(
        &self,
        Parameters(CreateTasksRequest { project_id, tasks }): Parameters<CreateTasksRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if tasks.is_empty() {
            return Self::err(
                "At least one task must be provided when creating tasks".to_string(),
                None::<String>,
            );
        }

        let mut created = Vec::new();
        let mut failed = Vec::new();
        let url = self.url("/api/tasks");

        for (idx, task_input) in tasks.into_iter().enumerate() {
            let title = task_input.title.trim().to_string();
            if title.is_empty() {
                failed.push(BatchOperationError {
                    identifier: format!("index {idx}"),
                    error: "Task title cannot be empty".to_string(),
                });
                continue;
            }

            let expanded_description = match task_input.description {
                Some(desc) => Some(self.expand_tags(&desc).await),
                None => None,
            };

            let payload =
                CreateTask::from_title_description(project_id, title.clone(), expanded_description);

            match self
                .send_json::<Task>(self.client.post(&url).json(&payload))
                .await
            {
                Ok(task) => created.push(CreatedTaskSummary {
                    task_id: task.id.to_string(),
                    title: task.title,
                }),
                Err(e) => failed.push(BatchOperationError {
                    identifier: format!("index {idx}"),
                    error: TaskServer::summarize_error(e),
                }),
            }
        }

        let response = CreateTasksResponse {
            count: created.len(),
            tasks: created,
            failed,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Create one or many projects and link at least one local git repository per project. Provide an array of projects with names and repositories."
    )]
    async fn create_projects(
        &self,
        Parameters(CreateProjectsRequest { projects }): Parameters<CreateProjectsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if projects.is_empty() {
            return Self::err(
                "At least one project must be provided when creating projects".to_string(),
                None::<String>,
            );
        }

        let mut created = Vec::new();
        let mut failed = Vec::new();
        let url = self.url("/api/projects");

        for (idx, project_input) in projects.into_iter().enumerate() {
            let trimmed_name = project_input.name.trim();
            if trimmed_name.is_empty() {
                failed.push(BatchOperationError {
                    identifier: format!("index {idx}"),
                    error: "Project name cannot be empty".to_string(),
                });
                continue;
            }

            if project_input.repositories.is_empty() {
                failed.push(BatchOperationError {
                    identifier: format!("index {idx}"),
                    error: "At least one repository is required when creating a project"
                        .to_string(),
                });
                continue;
            }

            let repo_payload: Vec<CreateProjectRepo> = project_input
                .repositories
                .into_iter()
                .map(|repo| CreateProjectRepo {
                    display_name: repo.display_name.trim().to_string(),
                    git_repo_path: repo.git_repo_path.trim().to_string(),
                })
                .collect();

            if repo_payload
                .iter()
                .any(|repo| repo.display_name.is_empty() || repo.git_repo_path.is_empty())
            {
                failed.push(BatchOperationError {
                    identifier: format!("index {idx}"),
                    error: "Each repository must include both a display_name and git_repo_path"
                        .to_string(),
                });
                continue;
            }

            let payload = CreateProject {
                name: trimmed_name.to_string(),
                repositories: repo_payload,
            };

            match self
                .send_json::<Project>(self.client.post(&url).json(&payload))
                .await
            {
                Ok(project) => created.push(CreateProjectResponse {
                    repository_count: payload.repositories.len(),
                    project: ProjectSummary::from_project(project),
                }),
                Err(e) => failed.push(BatchOperationError {
                    identifier: format!("index {idx}"),
                    error: TaskServer::summarize_error(e),
                }),
            }
        }

        let response = CreateProjectsResponse {
            count: created.len(),
            projects: created,
            failed,
        };

        TaskServer::success(&response)
    }

    #[tool(description = "List all the available projects")]
    async fn list_projects(&self) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/projects");
        let projects: Vec<Project> = match self.send_json(self.client.get(&url)).await {
            Ok(ps) => ps,
            Err(e) => return Ok(e),
        };

        let project_summaries: Vec<ProjectSummary> = projects
            .into_iter()
            .map(ProjectSummary::from_project)
            .collect();

        let response = ListProjectsResponse {
            count: project_summaries.len(),
            projects: project_summaries,
        };

        TaskServer::success(&response)
    }

    #[tool(description = "List all repositories for a project. `project_id` is required!")]
    async fn list_repos(
        &self,
        Parameters(ListReposRequest { project_id }): Parameters<ListReposRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!("/api/projects/{}/repositories", project_id));
        let repos: Vec<Repo> = match self.send_json(self.client.get(&url)).await {
            Ok(rs) => rs,
            Err(e) => return Ok(e),
        };

        let repo_summaries: Vec<McpRepoSummary> = repos
            .into_iter()
            .map(|r| McpRepoSummary {
                id: r.id.to_string(),
                name: r.name,
            })
            .collect();

        let response = ListReposResponse {
            count: repo_summaries.len(),
            repos: repo_summaries,
            project_id: project_id.to_string(),
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "List all the task/tickets in a project with optional filtering and execution status. `project_id` is required!"
    )]
    async fn list_tasks(
        &self,
        Parameters(ListTasksRequest {
            project_id,
            status,
            limit,
        }): Parameters<ListTasksRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let status_filter = if let Some(ref status_str) = status {
            match TaskStatus::from_str(status_str) {
                Ok(s) => Some(s),
                Err(_) => {
                    return Self::err(
                        "Invalid status filter. Valid values: 'todo', 'inprogress', 'inreview', 'done', 'cancelled'".to_string(),
                        Some(status_str.to_string()),
                    );
                }
            }
        } else {
            None
        };

        let url = self.url(&format!("/api/tasks?project_id={}", project_id));
        let all_tasks: Vec<TaskWithAttemptStatus> =
            match self.send_json(self.client.get(&url)).await {
                Ok(t) => t,
                Err(e) => return Ok(e),
            };

        let task_limit = limit.unwrap_or(50).max(0) as usize;
        let filtered = all_tasks.into_iter().filter(|t| {
            if let Some(ref want) = status_filter {
                &t.status == want
            } else {
                true
            }
        });
        let limited: Vec<TaskWithAttemptStatus> = filtered.take(task_limit).collect();

        let task_summaries: Vec<TaskSummary> = limited
            .into_iter()
            .map(TaskSummary::from_task_with_status)
            .collect();

        let response = ListTasksResponse {
            count: task_summaries.len(),
            tasks: task_summaries,
            project_id: project_id.to_string(),
            applied_filters: ListTasksFilters {
                status: status.clone(),
                limit: task_limit as i32,
            },
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "List tasks grouped by status with merge status. `project_id` is required!"
    )]
    async fn list_tasks_by_status(
        &self,
        Parameters(ListTasksByStatusRequest { project_id, limit }): Parameters<
            ListTasksByStatusRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        let mut url = self.url(&format!("/api/tasks/by-status?project_id={}", project_id));
        if let Some(limit) = limit {
            url.push_str(&format!("&limit={}", limit));
        }
        let groups: Vec<ApiTasksByStatusGroup> = match self.send_json(self.client.get(&url)).await {
            Ok(g) => g,
            Err(e) => return Ok(e),
        };

        let mut total_count = 0usize;
        let output_groups: Vec<TasksByStatusGroup> = groups
            .into_iter()
            .map(|group| {
                let tasks: Vec<TaskWithMergeSummary> = group
                    .tasks
                    .into_iter()
                    .map(|task| {
                        total_count += 1;
                        TaskWithMergeSummary {
                            id: task.id.to_string(),
                            title: task.title,
                            status: task.status.to_string(),
                            updated_at: task.updated_at.to_rfc3339(),
                            is_merged: task.is_merged,
                        }
                    })
                    .collect();

                TasksByStatusGroup {
                    status: group.status.to_string(),
                    tasks,
                }
            })
            .collect();

        let response = ListTasksByStatusResponse {
            project_id: project_id.to_string(),
            groups: output_groups,
            count: total_count,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Start working on a task by creating and launching a new workspace session. Supported executors: CLAUDE_CODE, AMP, GEMINI, CODEX, OPENCODE, CURSOR_AGENT, QWEN_CODE, COPILOT, DROID."
    )]
    async fn start_workspace_session(
        &self,
        Parameters(StartWorkspaceSessionRequest {
            task_id,
            executor,
            variant,
            repos,
        }): Parameters<StartWorkspaceSessionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if repos.is_empty() {
            return Self::err(
                "At least one repository must be specified.".to_string(),
                None::<String>,
            );
        }

        let executor_trimmed = executor.trim();
        if executor_trimmed.is_empty() {
            return Self::err("Executor must not be empty.".to_string(), None::<String>);
        }

        let normalized_executor = executor_trimmed.replace('-', "_").to_ascii_uppercase();
        let base_executor = match BaseCodingAgent::from_str(&normalized_executor) {
            Ok(exec) => exec,
            Err(_) => {
                let options = "Supported executors: CLAUDE_CODE, AMP, GEMINI, CODEX, OPENCODE, CURSOR_AGENT, QWEN_CODE, COPILOT, DROID";
                return Self::err(
                    format!("Unknown executor '{executor_trimmed}'. {options}"),
                    None::<String>,
                );
            }
        };

        let variant = variant.and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let executor_profile_id = ExecutorProfileId {
            executor: base_executor,
            variant,
        };

        let workspace_repos: Vec<WorkspaceRepoInput> = repos
            .into_iter()
            .map(|r| WorkspaceRepoInput {
                repo_id: r.repo_id,
                target_branch: r.base_branch,
            })
            .collect();

        let payload = CreateTaskAttemptBody {
            task_id,
            executor_profile_id,
            repos: workspace_repos,
        };

        let url = self.url("/api/task-attempts");
        let workspace: Workspace = match self.send_json(self.client.post(&url).json(&payload)).await
        {
            Ok(workspace) => workspace,
            Err(e) => return Ok(e),
        };

        let response = StartWorkspaceSessionResponse {
            task_id: workspace.task_id.to_string(),
            workspace_id: workspace.id.to_string(),
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Start working on many tasks by creating and launching workspace sessions in bulk. Supported executors: CLAUDE_CODE, AMP, GEMINI, CODEX, OPENCODE, CURSOR_AGENT, QWEN_CODE, COPILOT, DROID."
    )]
    async fn start_workspace_sessions(
        &self,
        Parameters(StartWorkspaceSessionsRequest { sessions }): Parameters<
            StartWorkspaceSessionsRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        if sessions.is_empty() {
            return Self::err(
                "At least one session must be provided when starting workspaces".to_string(),
                None::<String>,
            );
        }

        let mut started = Vec::new();
        let mut failed = Vec::new();

        for session in sessions {
            let executor_trimmed = session.executor.trim();
            if executor_trimmed.is_empty() {
                failed.push(BatchOperationError {
                    identifier: session.task_id.to_string(),
                    error: "Executor must not be empty.".to_string(),
                });
                continue;
            }

            if session.repos.is_empty() {
                failed.push(BatchOperationError {
                    identifier: session.task_id.to_string(),
                    error: "At least one repository must be specified.".to_string(),
                });
                continue;
            }

            let normalized_executor = executor_trimmed.replace('-', "_").to_ascii_uppercase();
            let base_executor = match BaseCodingAgent::from_str(&normalized_executor) {
                Ok(exec) => exec,
                Err(_) => {
                    let options = "Supported executors: CLAUDE_CODE, AMP, GEMINI, CODEX, OPENCODE, CURSOR_AGENT, QWEN_CODE, COPILOT, DROID";
                    failed.push(BatchOperationError {
                        identifier: session.task_id.to_string(),
                        error: format!("Unknown executor '{executor_trimmed}'. {options}"),
                    });
                    continue;
                }
            };

            let variant = session.variant.and_then(|v| {
                let trimmed = v.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });

            let executor_profile_id = ExecutorProfileId {
                executor: base_executor,
                variant,
            };

            let workspace_repos: Vec<WorkspaceRepoInput> = session
                .repos
                .into_iter()
                .map(|r| WorkspaceRepoInput {
                    repo_id: r.repo_id,
                    target_branch: r.base_branch,
                })
                .collect();

            let payload = CreateTaskAttemptBody {
                task_id: session.task_id,
                executor_profile_id,
                repos: workspace_repos,
            };

            let url = self.url("/api/task-attempts");
            match self
                .send_json::<Workspace>(self.client.post(&url).json(&payload))
                .await
            {
                Ok(workspace) => started.push(StartWorkspaceSessionResponse {
                    task_id: workspace.task_id.to_string(),
                    workspace_id: workspace.id.to_string(),
                }),
                Err(e) => failed.push(BatchOperationError {
                    identifier: session.task_id.to_string(),
                    error: TaskServer::summarize_error(e),
                }),
            }
        }

        let response = StartWorkspaceSessionsResponse {
            count: started.len(),
            sessions: started,
            failed,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Update one or many tasks' title, description, or status. Each item requires `task_id`; `title`, `description`, and `status` are optional."
    )]
    async fn update_tasks(
        &self,
        Parameters(UpdateTasksRequest { tasks }): Parameters<UpdateTasksRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if tasks.is_empty() {
            return Self::err(
                "At least one task update must be provided".to_string(),
                None::<String>,
            );
        }

        let mut updated = Vec::new();
        let mut failed = Vec::new();

        for task_input in tasks {
            let status = if let Some(ref status_str) = task_input.status {
                match TaskStatus::from_str(status_str) {
                    Ok(s) => Some(s),
                    Err(_) => {
                        failed.push(BatchOperationError {
                            identifier: task_input.task_id.to_string(),
                            error: "Invalid status. Valid: 'todo', 'inprogress', 'inreview', 'done', 'cancelled'".to_string(),
                        });
                        continue;
                    }
                }
            } else {
                None
            };

            let expanded_description = match task_input.description {
                Some(desc) => Some(self.expand_tags(&desc).await),
                None => None,
            };

            let payload = UpdateTask {
                title: task_input.title,
                description: expanded_description,
                status,
                parent_workspace_id: None,
                image_ids: None,
            };

            let url = self.url(&format!("/api/tasks/{}", task_input.task_id));
            match self.send_json(self.client.put(&url).json(&payload)).await {
                Ok(task) => updated.push(TaskDetails::from_task(task)),
                Err(e) => failed.push(BatchOperationError {
                    identifier: task_input.task_id.to_string(),
                    error: TaskServer::summarize_error(e),
                }),
            }
        }

        let response = UpdateTasksResponse {
            count: updated.len(),
            tasks: updated,
            failed,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Delete one or many tasks/tickets from a project. Provide the array of task_ids to delete."
    )]
    async fn delete_tasks(
        &self,
        Parameters(DeleteTasksRequest { task_ids }): Parameters<DeleteTasksRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if task_ids.is_empty() {
            return Self::err(
                "At least one task_id must be provided when deleting tasks".to_string(),
                None::<String>,
            );
        }

        let mut deleted = Vec::new();
        let mut failed = Vec::new();

        for task_id in task_ids {
            let url = self.url(&format!("/api/tasks/{}", task_id));
            match self
                .send_json::<serde_json::Value>(self.client.delete(&url))
                .await
            {
                Ok(_) => deleted.push(task_id.to_string()),
                Err(e) => failed.push(BatchOperationError {
                    identifier: task_id.to_string(),
                    error: TaskServer::summarize_error(e),
                }),
            }
        }

        let response = DeleteTasksResponse {
            count: deleted.len(),
            deleted_task_ids: deleted,
            failed,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Update one or many projects. Each item requires `project_id`; `name`, `dev_script`, `dev_script_working_dir`, and `default_agent_working_dir` are optional."
    )]
    async fn update_projects(
        &self,
        Parameters(UpdateProjectsRequest { projects }): Parameters<UpdateProjectsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if projects.is_empty() {
            return Self::err(
                "At least one project update must be provided".to_string(),
                None::<String>,
            );
        }

        let mut updated = Vec::new();
        let mut failed = Vec::new();

        for project_input in projects {
            if project_input
                .name
                .as_deref()
                .map(str::trim)
                .map_or(false, |n| n.is_empty())
            {
                failed.push(BatchOperationError {
                    identifier: project_input.project_id.to_string(),
                    error: "Project name cannot be empty when provided".to_string(),
                });
                continue;
            }

            let payload = UpdateProject {
                name: project_input.name.map(|n| n.trim().to_string()),
                dev_script: project_input.dev_script,
                dev_script_working_dir: project_input.dev_script_working_dir,
                default_agent_working_dir: project_input.default_agent_working_dir,
            };

            let url = self.url(&format!("/api/projects/{}", project_input.project_id));
            match self
                .send_json::<Project>(self.client.put(&url).json(&payload))
                .await
            {
                Ok(project) => updated.push(ProjectSummary::from_project(project)),
                Err(e) => failed.push(BatchOperationError {
                    identifier: project_input.project_id.to_string(),
                    error: TaskServer::summarize_error(e),
                }),
            }
        }

        let response = UpdateProjectsResponse {
            count: updated.len(),
            projects: updated,
            failed,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Delete one or many projects. Provide the array of project_ids to delete."
    )]
    async fn delete_projects(
        &self,
        Parameters(DeleteProjectsRequest { project_ids }): Parameters<DeleteProjectsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if project_ids.is_empty() {
            return Self::err(
                "At least one project_id must be provided when deleting projects".to_string(),
                None::<String>,
            );
        }

        let mut deleted = Vec::new();
        let mut failed = Vec::new();

        for project_id in project_ids {
            let url = self.url(&format!("/api/projects/{}", project_id));
            match self
                .send_json::<serde_json::Value>(self.client.delete(&url))
                .await
            {
                Ok(_) => deleted.push(project_id.to_string()),
                Err(e) => failed.push(BatchOperationError {
                    identifier: project_id.to_string(),
                    error: TaskServer::summarize_error(e),
                }),
            }
        }

        let response = DeleteProjectsResponse {
            count: deleted.len(),
            deleted_project_ids: deleted,
            failed,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Get detailed information (like task description) about one or many tasks/tickets. You can use `list_tasks` to find task_ids."
    )]
    async fn get_tasks(
        &self,
        Parameters(GetTasksRequest { task_ids }): Parameters<GetTasksRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if task_ids.is_empty() {
            return Self::err(
                "At least one task_id must be provided when fetching tasks".to_string(),
                None::<String>,
            );
        }

        let mut tasks_out = Vec::new();
        let mut failed = Vec::new();

        for task_id in task_ids {
            let url = self.url(&format!("/api/tasks/{}", task_id));
            match self.send_json(self.client.get(&url)).await {
                Ok(task) => tasks_out.push(TaskDetails::from_task(task)),
                Err(e) => failed.push(BatchOperationError {
                    identifier: task_id.to_string(),
                    error: TaskServer::summarize_error(e),
                }),
            }
        }

        let response = GetTasksResponse {
            count: tasks_out.len(),
            tasks: tasks_out,
            failed,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = "Fetch the code diff for a task attempt. Provide `attempt_id` or set `latest=true` to use the newest attempt (uses the active task context when available). Set `include_stats` to add aggregated additions/deletions."
    )]
    async fn get_attempt_diff(
        &self,
        Parameters(GetAttemptDiffRequest {
            attempt_id,
            latest,
            include_stats,
        }): Parameters<GetAttemptDiffRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let use_latest = latest.unwrap_or(attempt_id.is_none());
        let attempt_id = match self.resolve_attempt_id(attempt_id, use_latest).await {
            Ok(id) => id,
            Err(err) => return Ok(err),
        };

        let mut url = self.url(&format!("/api/task-attempts/{}/diff", attempt_id));
        if include_stats.unwrap_or(false) {
            url.push_str("?include_stats=true");
        }

        let diff = match self
            .send_json::<TaskAttemptDiffResponse>(self.client.get(&url))
            .await
        {
            Ok(d) => d,
            Err(err) => return Ok(err),
        };

        TaskServer::success(&diff)
    }
}

#[tool_handler]
impl ServerHandler for TaskServer {
    fn get_info(&self) -> ServerInfo {
        let mut instruction = "A task and project management server. If you need to create or update tickets or tasks then use these tools. Most of them absolutely require that you pass the `project_id` of the project that you are currently working on. You can get project ids by using `list_projects`. Call `list_tasks` to fetch the `task_ids` of all the tasks in a project`. TOOLS: 'list_projects', 'create_projects', 'update_projects', 'delete_projects', 'list_tasks', 'list_tasks_by_status', 'create_tasks', 'start_workspace_session', 'start_workspace_sessions', 'get_tasks', 'get_attempt_diff', 'update_tasks', 'delete_tasks', 'list_repos'. Make sure to pass `project_id` or `task_id` where required. You can use list tools to get the available ids.".to_string();
        if self.context.is_some() {
            let context_instruction = "Use 'get_context' to fetch project/task/workspace metadata for the active Vibe Kanban workspace session when available.";
            instruction = format!("{} {}", context_instruction, instruction);
        }

        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "vibe-kanban".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some(instruction),
        }
    }
}
