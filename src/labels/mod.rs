#![allow(dead_code)]

use anyhow::{Result, bail, ensure};
use std::collections::HashMap;

pub const LABEL_ENABLE: &str = "dockguard.enable";
pub const LABEL_SCHEDULE: &str = "dockguard.schedule";
pub const LABEL_INTERVAL: &str = "dockguard.interval";
pub const LABEL_STOP_TIMEOUT: &str = "dockguard.stop-timeout";
pub const LABEL_WATCH: &str = "dockguard.watch";

const DEFAULT_INTERVAL_SECS: u64 = 86400;
const DEFAULT_STOP_TIMEOUT_SECS: u64 = 10;

const KNOWN_LABELS: &[&str] = &[
    LABEL_ENABLE,
    LABEL_SCHEDULE,
    LABEL_INTERVAL,
    LABEL_STOP_TIMEOUT,
    LABEL_WATCH,
];

#[derive(Debug, PartialEq, Clone)]
pub enum UpdateTrigger {
    Interval(u64),
    Schedule(String),
}

/// Raw label values parsed from Docker container labels.
///
/// `unknown_labels` contains any `dockguard.*` keys not recognised by this version.
/// Callers should log each entry at INFO level before discarding them.
#[derive(Debug)]
pub struct ContainerLabels {
    pub enable: Option<bool>,
    pub schedule: Option<String>,
    pub interval: Option<u64>,
    pub stop_timeout: Option<u64>,
    pub watch: Option<bool>,
    pub unknown_labels: Vec<String>,
}

/// Fully resolved per-container configuration after applying defaults.
#[derive(Debug, Clone)]
pub struct ResolvedContainerConfig {
    pub enabled: bool,
    pub update_trigger: UpdateTrigger,
    pub stop_timeout: u64,
    pub watch: bool,
}

impl ContainerLabels {
    /// Parse container labels from a Docker label map.
    pub fn from_map(labels: &HashMap<String, String>) -> Result<Self> {
        let enable = labels
            .get(LABEL_ENABLE)
            .map(|v| parse_bool(v, LABEL_ENABLE))
            .transpose()?;

        let schedule = labels.get(LABEL_SCHEDULE).cloned();

        let interval = labels
            .get(LABEL_INTERVAL)
            .map(|v| {
                v.parse::<u64>().map_err(|_| {
                    anyhow::anyhow!(
                        "Label '{}' must be a positive integer, got '{}'",
                        LABEL_INTERVAL,
                        v
                    )
                })
            })
            .transpose()?;

        let stop_timeout = labels
            .get(LABEL_STOP_TIMEOUT)
            .map(|v| {
                v.parse::<u64>().map_err(|_| {
                    anyhow::anyhow!(
                        "Label '{}' must be a positive integer, got '{}'",
                        LABEL_STOP_TIMEOUT,
                        v
                    )
                })
            })
            .transpose()?;

        let watch = labels
            .get(LABEL_WATCH)
            .map(|v| parse_bool(v, LABEL_WATCH))
            .transpose()?;

        let unknown_labels: Vec<String> = labels
            .keys()
            .filter(|k| k.starts_with("dockguard.") && !KNOWN_LABELS.contains(&k.as_str()))
            .cloned()
            .collect();

        Ok(Self {
            enable,
            schedule,
            interval,
            stop_timeout,
            watch,
            unknown_labels,
        })
    }

    /// Resolve final container config by applying global enable mode and defaults.
    ///
    /// `global_enable`: true = opt-in (container needs dockguard.enable=true to be managed),
    ///                  false = opt-out (container is managed unless dockguard.enable=false).
    pub fn resolve(self, global_enable: bool) -> Result<ResolvedContainerConfig> {
        let enabled = self.enable.unwrap_or(!global_enable);

        let update_trigger = match (self.schedule, self.interval) {
            (Some(cron), None) => {
                let normalized_cron = validate_cron(&cron)?;
                UpdateTrigger::Schedule(normalized_cron)
            }
            (None, Some(secs)) => {
                ensure!(
                    secs > 0,
                    "Label '{}' must be greater than 0",
                    LABEL_INTERVAL
                );
                UpdateTrigger::Interval(secs)
            }
            (None, None) => UpdateTrigger::Interval(DEFAULT_INTERVAL_SECS),
            (Some(_), Some(_)) => {
                bail!(
                    "Labels '{}' and '{}' are mutually exclusive",
                    LABEL_SCHEDULE,
                    LABEL_INTERVAL
                )
            }
        };

        let stop_timeout = self.stop_timeout.unwrap_or(DEFAULT_STOP_TIMEOUT_SECS);
        ensure!(
            stop_timeout > 0,
            "Label '{}' must be greater than 0",
            LABEL_STOP_TIMEOUT
        );

        Ok(ResolvedContainerConfig {
            enabled,
            update_trigger,
            stop_timeout,
            watch: self.watch.unwrap_or(false),
        })
    }
}

fn parse_bool(value: &str, label: &str) -> Result<bool> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => bail!(
            "Label '{}' must be 'true' or 'false', got '{}'",
            label,
            value
        ),
    }
}

fn validate_cron(expr: &str) -> Result<String> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    ensure!(
        parts.len() == 5 || parts.len() == 6,
        "Invalid cron expression '{}': expected 5 or 6 fields, got {}",
        expr,
        parts.len()
    );
    Ok(if parts.len() == 5 {
        format!("0 {expr} *")
    } else {
        format!("{expr} *")
    })
}

#[cfg(test)]
mod tests;
