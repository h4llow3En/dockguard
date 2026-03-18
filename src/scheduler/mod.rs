use crate::config::ValidatedConfig;
use crate::labels::UpdateTrigger;
use crate::updater::perform_update;
use crate::watcher::ManagedContainer;
use bollard::Docker;
use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable {
        local: String,
        remote: String,
        local_only: bool,
    },
}

/// A job to be processed by the [`update_worker`].
pub struct UpdateJob {
    pub docker: Docker,
    pub container: ManagedContainer,
    pub pull_timeout: u64,
    pub clean: bool,
}

pub type UpdateSender = mpsc::Sender<UpdateJob>;

/// Flag set when a self-update has been queued.
///
/// Once set, schedulers stop queuing new jobs. Already-queued jobs are still
/// processed before the self-update runs.
pub type SelfUpdatePending = Arc<AtomicBool>;

pub async fn run(
    docker: Docker,
    container: ManagedContainer,
    cfg: Arc<ValidatedConfig>,
    tx: UpdateSender,
    is_self: bool,
    self_update_pending: SelfUpdatePending,
) {
    tracing::debug!("Scheduler started for container {}", container.name);
    loop {
        tokio::select! {
            _ = container.cancel_token.cancelled() => {
                tracing::debug!("Scheduler stopped for container {}", container.name);
                break;
            }
            _ = trigger_wait(&container.config.update_trigger) => {
                if self_update_pending.load(Ordering::Relaxed) {
                    tracing::info!(
                        "Self-update pending — stopping scheduler for {}",
                        container.name
                    );
                    break;
                }

                if is_self {
                    self_update_pending.store(true, Ordering::Relaxed);
                }

                let job = UpdateJob {
                    docker: docker.clone(),
                    container: container.clone(),
                    pull_timeout: cfg.pull_timeout,
                    clean: cfg.clean,
                };
                if tx.send(job).await.is_err() {
                    tracing::error!(
                        "Update queue closed — stopping scheduler for {}",
                        container.name
                    );
                    break;
                }

                if is_self {
                    tracing::info!(
                        "Self-update queued — scheduler for {} exiting",
                        container.name
                    );
                    break;
                }
            }
        }
    }
}

/// Processes queued update jobs sequentially, one at a time.
pub async fn update_worker(mut rx: mpsc::Receiver<UpdateJob>) {
    while let Some(job) = rx.recv().await {
        tracing::debug!(
            "Checking for updates for container {} (image: {})",
            job.container.name,
            job.container.image
        );
        match check(&job.docker, &job.container).await {
            Some(UpdateStatus::UpdateAvailable {
                local,
                remote,
                local_only,
            }) => {
                if job.container.config.watch {
                    tracing::info!(
                        container = %job.container.name,
                        image = %job.container.image,
                        local = %local,
                        remote = %remote,
                        "Image update available"
                    );
                } else {
                    tracing::info!(
                        container = %job.container.name,
                        image = %job.container.image,
                        local = %local,
                        remote = %remote,
                        "Image update available — performing update"
                    );
                    if let Err(e) = perform_update(
                        &job.docker,
                        &job.container,
                        job.pull_timeout,
                        job.clean,
                        local_only,
                    )
                    .await
                    {
                        tracing::error!("Update of container {} failed: {e:#}", job.container.name);
                    }
                }
            }
            Some(UpdateStatus::UpToDate) => {
                tracing::debug!(
                    "Container {} — image {} is up to date",
                    job.container.name,
                    job.container.image
                );
            }
            None => {} // error already logged in check
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

/// Checks whether a newer image is available for the container.
///
/// Two-phase comparison to minimise registry requests:
/// **Local**: compare the image ID the container is running against the
///            image ID the tag currently resolves to.  If they differ a newer version
///            has already been pulled locally — no registry call needed.
/// **Registry**: if the local image is up to date, compare its manifest
///               digest against the registry to detect remote-only updates.
///
/// Returns `None` if the check could not be completed.
pub async fn check(docker: &Docker, container: &ManagedContainer) -> Option<UpdateStatus> {
    let tagged_image = match docker.inspect_image(&container.image).await {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!("Could not inspect local image {}: {e:#}", container.image);
            return None;
        }
    };

    let tagged_id = tagged_image.id.as_deref().unwrap_or_default();
    if tagged_id != container.image_id {
        tracing::debug!(
            "Container {} local image ID mismatch: running={}, tagged={}",
            container.name,
            container.image_id,
            tagged_id
        );
        return Some(UpdateStatus::UpdateAvailable {
            local: container.image_id.clone(),
            remote: tagged_id.to_string(),
            local_only: true,
        });
    }

    let local_digest = tagged_image
        .repo_digests
        .as_ref()
        .and_then(|digests| {
            digests
                .iter()
                .find_map(|d| d.split_once('@').map(|(_, digest)| digest.to_string()))
        })
        .or_else(|| {
            tracing::warn!(
                "Image {} has no repo digest — cannot compare with registry (locally built?)",
                container.image
            );
            None
        })?;

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
            local_only: false,
        })
    } else {
        Some(UpdateStatus::UpToDate)
    }
}

#[cfg(test)]
mod tests;
