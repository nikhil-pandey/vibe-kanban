//! Routes for the unified task dashboard - fetches all tasks across all projects

use axum::{
    Router,
    extract::{State, ws::{WebSocket, WebSocketUpgrade}},
    response::{IntoResponse, Json as ResponseJson},
    routing::get,
};
use db::models::task::{Task, TaskWithAttemptStatusAndProject};
use deployment::Deployment;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use utils::log_msg::LogMsg;
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError};

/// GET /api/all-tasks - Get all tasks across all projects
pub async fn get_all_tasks(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<TaskWithAttemptStatusAndProject>>>, ApiError> {
    let tasks = Task::find_all_with_attempt_status_and_project(&deployment.db().pool).await?;
    Ok(ResponseJson(ApiResponse::success(tasks)))
}

/// WebSocket endpoint for streaming all tasks across all projects
pub async fn stream_all_tasks_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_all_tasks_ws(socket, deployment).await {
            tracing::warn!("all-tasks WS closed: {}", e);
        }
    })
}

async fn handle_all_tasks_ws(
    socket: WebSocket,
    deployment: DeploymentImpl,
) -> anyhow::Result<()> {
    // Get the raw stream and convert LogMsg to WebSocket messages
    let mut stream = deployment
        .events()
        .stream_all_tasks_raw()
        .await?
        .map_ok(|msg: LogMsg| msg.to_ws_message_unchecked());

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Drain (and ignore) any client->server messages so pings/pongs work
    tokio::spawn(async move { while let Some(Ok(_)) = receiver.next().await {} });

    // Forward server messages
    while let Some(item) = stream.next().await {
        match item {
            Ok(msg) => {
                if sender.send(msg).await.is_err() {
                    break; // client disconnected
                }
            }
            Err(e) => {
                tracing::error!("all-tasks stream error: {}", e);
                break;
            }
        }
    }
    Ok(())
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/all-tasks", get(get_all_tasks))
        .route("/all-tasks/stream/ws", get(stream_all_tasks_ws))
}
