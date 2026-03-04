use anyhow::{Result, bail, ensure};
use clap::Parser;

#[allow(dead_code)]
#[derive(Debug)]
pub enum UpdateTrigger {
    Interval(u64),
    Schedule(String),
}

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

    /// Cron expression for the update schedule (mutually exclusive with --intervall)
    #[arg(
        short = 's',
        long,
        env = "GUARD_SCHEDULE",
        conflicts_with = "intervall"
    )]
    pub schedule: Option<String>,

    /// Interval in seconds between update checks (mutually exclusive with --schedule).
    /// Default: 86400 (24 hours)
    #[arg(
        short = 'i',
        long,
        env = "GUARD_INTERVALL",
        conflicts_with = "schedule"
    )]
    pub intervall: Option<u64>,

    /// Only watch for updates and log them without actually performing updates
    #[arg(long, env = "GUARD_WATCH", default_value_t = false)]
    pub watch: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "GUARD_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Timeout in seconds for image pulls
    #[arg(long, env = "GUARD_PULL_TIMEOUT", default_value_t = 300)]
    pub pull_timeout: u64,

    /// Seconds to wait for a container to stop gracefully before SIGKILL
    #[arg(long, env = "GUARD_STOP_TIMEOUT", default_value_t = 10)]
    pub stop_timeout: u64,

    /// Run once and exit instead of running as a daemon
    #[arg(long, default_value_t = false)]
    pub once: bool,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ValidatedConfig {
    pub clean: bool,
    pub host: Option<String>,
    pub update_trigger: UpdateTrigger,
    pub watch: bool,
    pub log_level: String,
    pub pull_timeout: u64,
    pub stop_timeout: u64,
    pub once: bool,
}

impl Config {
    pub fn validate(self) -> Result<ValidatedConfig> {
        let update_trigger = match (self.schedule, self.intervall) {
            (Some(cron), None) => UpdateTrigger::Schedule(cron),
            (None, Some(secs)) => UpdateTrigger::Interval(secs),
            (None, None) => UpdateTrigger::Interval(86400),
            (Some(_), Some(_)) => {
                bail!("Only one of --schedule and --intervall may be set at the same time")
            }
        };

        if let UpdateTrigger::Schedule(ref expr) = update_trigger {
            let parts: Vec<&str> = expr.split_whitespace().collect();
            ensure!(
                parts.len() == 5 || parts.len() == 6,
                "Invalid cron expression '{}': expected 5 or 6 fields, got {}",
                expr,
                parts.len()
            );
        }

        ensure!(
            self.pull_timeout > 0,
            "--pull-timeout must be greater than 0"
        );
        ensure!(
            self.stop_timeout > 0,
            "--stop-timeout must be greater than 0"
        );

        if let Some(ref host) = self.host {
            let valid_schemes = ["unix://", "tcp://", "http://", "https://"];
            ensure!(
                valid_schemes.iter().any(|s| host.starts_with(s)),
                "Invalid host scheme in '{}': expected one of unix://, tcp://, http://, https://",
                host
            );
        }

        Ok(ValidatedConfig {
            clean: self.clean,
            host: self.host,
            update_trigger,
            watch: self.watch,
            log_level: self.log_level,
            pull_timeout: self.pull_timeout,
            stop_timeout: self.stop_timeout,
            once: self.once,
        })
    }
}

#[cfg(test)]
mod tests;
