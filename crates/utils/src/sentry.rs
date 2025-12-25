use std::sync::OnceLock;

use sentry_tracing::{EventFilter, SentryLayer};

static INIT_GUARD: OnceLock<sentry::ClientInitGuard> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
pub enum SentrySource {
    Backend,
    Mcp,
}

// Telemetry disabled: keep API surface but make it a no-op.
pub fn init_once(_source: SentrySource) {
    let _ = &INIT_GUARD;
}

pub fn configure_user_scope(_user_id: &str, _username: Option<&str>, _email: Option<&str>) {
    // no-op
}

pub fn sentry_layer<S>() -> SentryLayer<S>
where
    S: tracing::Subscriber,
    S: for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    SentryLayer::default().event_filter(|_meta| EventFilter::Ignore)
}
