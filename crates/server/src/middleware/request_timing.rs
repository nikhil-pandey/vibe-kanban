use std::time::Instant;

use axum::{body::Body, http::Request, middleware::Next, response::Response};
use tracing::info;

/// Log duration for each API request.
pub async fn log_timing(req: Request<Body>, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();

    let response = next.run(req).await;
    let status = response.status();
    let elapsed_ms = start.elapsed().as_millis();

    info!(
        target: "server::http",
        %method,
        uri = %uri,
        status = %status,
        elapsed_ms,
        "Handled request"
    );

    response
}
