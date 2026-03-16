use crate::config::ValidatedConfig;
use crate::labels::UpdateTrigger;
use crate::updater::perform_update;
use crate::watcher::{ManagedContainer, UpdateGate};
use bollard::Docker;
use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable { local: String, remote: String },
}

pub async fn run(
    docker: Docker,
    container: ManagedContainer,
    cfg: Arc<ValidatedConfig>,
    gate: UpdateGate,
    is_self: bool,
) {
    tracing::debug!("Scheduler started for container {}", container.name);
    loop {
        tokio::select! {
            _ = container.cancel_token.cancelled() => {
                tracing::debug!("Scheduler stopped for container {}", container.name);
                break;
            }
            _ = trigger_wait(&container.config.update_trigger) => {
                tracing::debug!("Checking for updates for container {} (image: {})", container.name, container.image);
                match check(&docker, &container).await {
                    Some(UpdateStatus::UpdateAvailable { local, remote }) => {
                        if container.config.watch {
                            tracing::info!(
                                container = %container.name,
                                image = %container.image,
                                local = %local,
                                remote = %remote,
                                "Image update available"
                            );
                        } else {
                            tracing::info!(
                                container = %container.name,
                                image = %container.image,
                                local = %local,
                                remote = %remote,
                                "Image update available — triggering update"
                            );
                            // Self-updates acquire an exclusive write lock so all
                            // other in-progress updates finish first and no new
                            // ones can start while dockguard is replacing itself.
                            // Regular updates hold a shared read lock.
                            if is_self {
                                let _guard = gate.write().await;
                                if let Err(e) = perform_update(
                                    &docker,
                                    &container,
                                    cfg.pull_timeout,
                                    cfg.clean,
                                )
                                .await
                                {
                                    tracing::error!(
                                        "Self-update failed: {e:#}"
                                    );
                                }
                            } else {
                                let _guard = gate.read().await;
                                if let Err(e) = perform_update(
                                    &docker,
                                    &container,
                                    cfg.pull_timeout,
                                    cfg.clean,
                                )
                                .await
                                {
                                    tracing::error!(
                                        "Update of container {} failed: {e:#}",
                                        container.name
                                    );
                                }
                            }
                        }
                    }
                    Some(UpdateStatus::UpToDate) => {
                        tracing::debug!(
                            "Container {} — image {} is up to date",
                            container.name,
                            container.image
                        );
                    }
                    None => {} // error already logged in check
                }
            }
        }
    }
}

async fn trigger_wait(trigger: &UpdateTrigger) {
    tracing::debug!("Waiting for next trigger: {:?}", trigger);
    match trigger {
        UpdateTrigger::Interval(secs) => {
            tokio::time::sleep(Duration::from_secs(*secs)).await;
        }
        UpdateTrigger::Schedule(expr) => {
            let schedule = match Schedule::from_str(expr) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Invalid cron expression '{expr}': {e:#}");
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                    return;
                }
            };
            match schedule.upcoming(Utc).next() {
                Some(next) => {
                    let duration = (next - Utc::now())
                        .to_std()
                        .unwrap_or(Duration::from_secs(60));
                    tokio::time::sleep(duration).await;
                }
                None => {
                    tracing::error!("Cron schedule '{expr}' yields no upcoming events");
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                }
            }
        }
    }
}

/// Checks whether a newer image is available in the registry without pulling.
/// Returns `None` if the check could not be completed (errors are logged as `warn`).
pub async fn check(docker: &Docker, container: &ManagedContainer) -> Option<UpdateStatus> {
    let local_digest = get_local_digest(docker, &container.image).await?;

    let remote_digest = match docker.inspect_registry_image(&container.image, None).await {
        Ok(info) => match info.descriptor.digest {
            Some(d) => d,
            None => {
                tracing::warn!(
                    "Container {}: registry returned no digest for {}",
                    container.name,
                    container.image
                );
                return None;
            }
        },
        Err(e) => {
            tracing::warn!(
                "Container {}: registry check failed for {}: {e:#}",
                container.name,
                container.image
            );
            return None;
        }
    };

    if local_digest != remote_digest {
        Some(UpdateStatus::UpdateAvailable {
            local: local_digest,
            remote: remote_digest,
        })
    } else {
        Some(UpdateStatus::UpToDate)
    }
}

#[cfg(test)]
mod tests;

/// Extracts the manifest digest from the locally stored image metadata.
/// Returns `None` if the image has no repo digest (e.g. locally built images).
async fn get_local_digest(docker: &Docker, image: &str) -> Option<String> {
    let info = docker
        .inspect_image(image)
        .await
        .map_err(|e| tracing::warn!("Could not inspect local image {image}: {e:#}"))
        .ok()?;

    info.repo_digests?
        .into_iter()
        .find_map(|d| d.split_once('@').map(|(_, digest)| digest.to_string()))
        .or_else(|| {
            tracing::warn!("Image {image} has no repo digest — cannot compare (locally built?)");
            None
        })
}
