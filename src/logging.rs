use anyhow::{Context, Result};
use tracing::Level;
use tracing_subscriber::{
    Registry, fmt, fmt::time::ChronoLocal, layer::SubscriberExt as _, util::SubscriberInitExt as _,
};

/// Initialise the global tracing subscriber.
///
/// Installs a human-readable console layer.
pub fn init(level: Level) -> Result<()> {
    let fmt_layer = fmt::layer()
        .with_timer(ChronoLocal::new("%Y-%m-%d %H:%M:%S".to_string()))
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .compact();

    Registry::default()
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive(level.into()))
        .with(fmt_layer)
        .try_init()
        .context("Failed to initialise logging")
}
