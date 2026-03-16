use anyhow::{Context, Result, ensure};
use clap::Parser;
use std::sync::Arc;
use tracing::Level;

#[derive(Parser, Debug)]
#[command(
    name = "dockguard",
    version,
    about = "A Docker container update watcher"
)]
pub struct Config {
    /// Remove old images after updating a container
    #[arg(long, env = "GUARD_CLEAN", default_value_t = false)]
    pub clean: bool,

    /// Docker host (e.g. unix:///var/run/docker.sock or tcp://host:2375).
    /// When not set, connect_with_local_defaults() is used.
    #[arg(long, env = "DOCKER_HOST")]
    pub host: Option<String>,

    /// Enable mode: true = opt-in (only containers with dockguard.enable=true are managed),
    /// false = opt-out (all containers are managed unless dockguard.enable=false)
    #[arg(long, env = "GUARD_ENABLE", default_value_t = true)]
    pub enable: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "GUARD_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Timeout in seconds for image pulls
    #[arg(long, env = "GUARD_PULL_TIMEOUT", default_value_t = 300)]
    pub pull_timeout: u64,

    /// Run once and exit instead of running as a daemon
    #[arg(long, default_value_t = false)]
    pub once: bool,

    /// Internal flag to run the healthcheck
    #[arg(long, default_value_t = false, hide = true)]
    pub healthcheck: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ValidatedConfig {
    pub clean: bool,
    pub host: Option<String>,
    pub enable: bool,
    pub self_update: bool,
    pub log_level: Level,
    pub pull_timeout: u64,
    pub once: bool,
    pub healthcheck: bool,
}

impl Config {
    pub fn validate(self) -> Result<Arc<ValidatedConfig>> {
        ensure!(
            self.pull_timeout > 0,
            "--pull-timeout must be greater than 0"
        );

        let log_level = self.log_level.parse::<Level>().with_context(|| {
            format!(
                "Invalid log level '{}': expected one of trace, debug, info, warn, error",
                self.log_level
            )
        })?;

        let self_update = std::env::var("GUARD_SELF_UPDATE")
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);

        if let Some(ref host) = self.host {
            let valid_schemes = ["unix://", "tcp://", "http://", "https://"];
            ensure!(
                valid_schemes.iter().any(|s| host.starts_with(s)),
                "Invalid host scheme in '{}': expected one of unix://, tcp://, http://, https://",
                host
            );
        }

        Ok(Arc::new(ValidatedConfig {
            clean: self.clean,
            host: self.host,
            enable: self.enable,
            self_update,
            log_level,
            pull_timeout: self.pull_timeout,
            once: self.once,
            healthcheck: self.healthcheck,
        }))
    }
}

#[cfg(test)]
mod tests;
