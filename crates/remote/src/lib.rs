mod app;
mod auth;
pub mod config;
pub mod db;
pub mod github_app;
pub mod mail;
pub mod r2;
pub mod routes;
mod state;
pub mod validated_where;

use std::{env, sync::OnceLock};

pub use app::Server;
use sentry_tracing::{EventFilter, SentryLayer};
pub use state::AppState;
use tracing::Level;
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::{Layer as _, SubscriberExt},
    util::SubscriberInitExt,
};

static INIT_GUARD: OnceLock<sentry::ClientInitGuard> = OnceLock::new();

pub fn init_tracing() {
    if tracing::dispatcher::has_been_set() {
        return;
    }

    let env_filter = env::var("RUST_LOG").unwrap_or_else(|_| "info,sqlx=warn".to_string());
    let fmt_layer = fmt::layer()
        .json()
        .with_target(false)
        .with_span_events(FmtSpan::CLOSE)
        .boxed();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .with(ErrorLayer::default())
        .with(fmt_layer)
        // Telemetry disabled
        // .with(sentry_layer())
        .init();
}

fn environment() -> &'static str {
    if cfg!(debug_assertions) {
        "dev"
    } else {
        "production"
    }
}

// Telemetry disabled: keep signature but no-op.
pub fn sentry_init_once() {}

pub fn configure_user_scope(user_id: uuid::Uuid, username: Option<&str>, email: Option<&str>) {
    let _ = (user_id, username, email);
}

fn sentry_layer<S>() -> SentryLayer<S>
where
    S: tracing::Subscriber,
    S: for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    SentryLayer::default()
        .span_filter(|meta| {
            matches!(
                *meta.level(),
                Level::DEBUG | Level::INFO | Level::WARN | Level::ERROR
            )
        })
        .event_filter(|meta| match *meta.level() {
            Level::ERROR => EventFilter::Event,
            Level::DEBUG | Level::INFO | Level::WARN => EventFilter::Breadcrumb,
            Level::TRACE => EventFilter::Ignore,
        })
}
