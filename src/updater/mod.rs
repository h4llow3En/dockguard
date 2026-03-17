use crate::watcher::ManagedContainer;
use anyhow::{Context, Result};
use bollard::Docker;
use bollard::models::{
    ContainerCreateBody, ContainerState, ContainerStateStatusEnum, HealthStatusEnum,
    NetworkingConfig,
};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, RemoveContainerOptionsBuilder,
    StartContainerOptionsBuilder, StopContainerOptionsBuilder,
};
use futures_util::StreamExt;
use std::time::Duration;

const HEALTH_WAIT_TIMEOUT: Duration = Duration::from_secs(60);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Performs a full container update lifecycle:
/// pull → inspect → build config → stop → remove → recreate → start → health-check → (rollback).
///
/// The create-body is assembled from the inspection *before* any destructive step so that
/// config errors are caught while the container is still running.
///
/// If the new container fails its health check (or exits / stays in a restart loop),
/// a rollback to the previous image is attempted automatically.
pub async fn perform_update(
    docker: &Docker,
    container: &ManagedContainer,
    pull_timeout: u64,
    clean: bool,
    skip_pull: bool,
) -> Result<()> {
    let old_image_id = container.image_id.clone();

    if skip_pull {
        tracing::info!(
            "Newer image for {} already available locally — skipping pull",
            container.image
        );
    } else {
        pull_image(docker, &container.image, pull_timeout).await?;
    }

    let inspect = docker
        .inspect_container(&container.id, None)
        .await
        .context("Failed to inspect container before update")?;

    let name = inspect
        .name
        .as_deref()
        .unwrap_or(&container.name)
        .trim_start_matches('/')
        .to_string();

    let cfg = inspect
        .config
        .as_ref()
        .context("Container inspect returned no config")?;

    let networking = inspect
        .network_settings
        .as_ref()
        .and_then(|ns| ns.networks.clone())
        .map(|endpoints| NetworkingConfig {
            endpoints_config: Some(endpoints),
        });

    let body = ContainerCreateBody {
        hostname: cfg.hostname.clone(),
        domainname: cfg.domainname.clone(),
        user: cfg.user.clone(),
        attach_stdin: cfg.attach_stdin,
        attach_stdout: cfg.attach_stdout,
        attach_stderr: cfg.attach_stderr,
        exposed_ports: cfg.exposed_ports.clone(),
        tty: cfg.tty,
        open_stdin: cfg.open_stdin,
        stdin_once: cfg.stdin_once,
        env: cfg.env.clone(),
        cmd: cfg.cmd.clone(),
        healthcheck: cfg.healthcheck.clone(),
        args_escaped: cfg.args_escaped,
        image: cfg.image.clone(),
        volumes: cfg.volumes.clone(),
        working_dir: cfg.working_dir.clone(),
        entrypoint: cfg.entrypoint.clone(),
        network_disabled: cfg.network_disabled,
        on_build: cfg.on_build.clone(),
        labels: cfg.labels.clone(),
        stop_signal: cfg.stop_signal.clone(),
        stop_timeout: cfg.stop_timeout,
        shell: cfg.shell.clone(),
        host_config: inspect.host_config.clone(),
        networking_config: networking,
    };

    tracing::info!(
        "Stopping container {name} (stop-timeout: {}s)...",
        container.config.stop_timeout
    );
    if let Err(e) = docker
        .stop_container(
            &container.id,
            Some(
                StopContainerOptionsBuilder::default()
                    .t(container.config.stop_timeout as i32)
                    .build(),
            ),
        )
        .await
    {
        tracing::warn!("Failed to stop container {name}: {e:#} — attempting removal anyway");
    }

    docker
        .remove_container(
            &container.id,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await
        .context("Failed to remove old container")?;

    let created = docker
        .create_container(
            Some(CreateContainerOptionsBuilder::default().name(&name).build()),
            body.clone(),
        )
        .await
        .context("Failed to create new container")?;

    docker
        .start_container(
            &created.id,
            Some(StartContainerOptionsBuilder::default().build()),
        )
        .await
        .context("Failed to start new container")?;

    tracing::info!("Waiting for container {name} to become healthy...");
    match wait_healthy(docker, &created.id, HEALTH_WAIT_TIMEOUT).await {
        HealthWaitOutcome::Healthy => {
            tracing::info!(
                "Container {name} updated successfully (new id: {})",
                &created.id[..12.min(created.id.len())]
            );

            if clean {
                if let Err(e) = docker
                    .remove_image(
                        &old_image_id,
                        None::<bollard::query_parameters::RemoveImageOptions>,
                        None,
                    )
                    .await
                {
                    tracing::warn!("Failed to remove old image {old_image_id}: {e:#}");
                } else {
                    tracing::debug!("Removed old image {old_image_id}");
                }
            }
        }
        outcome => {
            let reason = match outcome {
                HealthWaitOutcome::Failed => "container exited or became unhealthy",
                HealthWaitOutcome::Timeout => "health check timed out",
                HealthWaitOutcome::Healthy => unreachable!(),
            };
            tracing::error!(
                "Container {name} failed after update ({reason}) — rolling back to previous image"
            );
            rollback(
                docker,
                &created.id,
                &name,
                body,
                &old_image_id,
                container.config.stop_timeout,
            )
            .await;
            anyhow::bail!("Update of {name} failed ({reason}); rollback attempted");
        }
    }

    Ok(())
}

pub(crate) enum HealthWaitOutcome {
    Healthy,
    Failed,
    Timeout,
}

/// Polls container state until it is confirmed healthy, clearly failed, or the timeout expires.
///
/// - Containers **with** a HEALTHCHECK: waits for `healthy` / `unhealthy`.
/// - Containers **without** a HEALTHCHECK (`Health == None` or status `none`):
///   considers the container healthy as soon as it is in `running` state.
async fn wait_healthy(docker: &Docker, id: &str, timeout: Duration) -> HealthWaitOutcome {
    let start = tokio::time::Instant::now();

    loop {
        if start.elapsed() >= timeout {
            return HealthWaitOutcome::Timeout;
        }

        let info = match docker.inspect_container(id, None).await {
            Ok(i) => i,
            Err(_) => return HealthWaitOutcome::Failed,
        };

        let Some(state) = info.state.as_ref() else {
            return HealthWaitOutcome::Failed;
        };

        if let Some(outcome) = classify_state(state) {
            return outcome;
        }

        tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
    }
}

/// Classifies a container state snapshot as terminal-healthy, terminal-failed, or still pending.
///
/// Returns `None` when the container is still transitioning and the caller should keep polling.
pub(crate) fn classify_state(state: &ContainerState) -> Option<HealthWaitOutcome> {
    // Exited / dead are always terminal failures regardless of health status.
    match state.status {
        Some(ContainerStateStatusEnum::EXITED) | Some(ContainerStateStatusEnum::DEAD) => {
            return Some(HealthWaitOutcome::Failed);
        }
        _ => {}
    }

    match state.health.as_ref().and_then(|h| h.status) {
        Some(HealthStatusEnum::HEALTHY) => Some(HealthWaitOutcome::Healthy),
        Some(HealthStatusEnum::UNHEALTHY) => Some(HealthWaitOutcome::Failed),
        // No HEALTHCHECK configured: healthy as soon as the container is running.
        Some(HealthStatusEnum::NONE) | None => {
            if state.status == Some(ContainerStateStatusEnum::RUNNING) {
                Some(HealthWaitOutcome::Healthy)
            } else {
                None // still starting up, keep waiting
            }
        }
        // STARTING or EMPTY: healthcheck is pending, keep waiting.
        _ => None,
    }
}

#[cfg(test)]
mod tests;

/// Attempts to roll back by stopping/removing the failed container and recreating
/// it from `old_image_id` with the same config.
///
/// All errors during rollback are logged but not propagated — the caller already
/// returns a failure result.
async fn rollback(
    docker: &Docker,
    failed_id: &str,
    name: &str,
    mut body: ContainerCreateBody,
    old_image_id: &str,
    stop_timeout: u64,
) {
    let _ = docker
        .stop_container(
            failed_id,
            Some(
                StopContainerOptionsBuilder::default()
                    .t(stop_timeout as i32)
                    .build(),
            ),
        )
        .await;

    if let Err(e) = docker
        .remove_container(
            failed_id,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await
    {
        tracing::error!("Rollback: Failed to remove failed container {name}: {e:#}");
        return;
    }

    body.image = Some(old_image_id.to_string());

    let created = match docker
        .create_container(
            Some(CreateContainerOptionsBuilder::default().name(name).build()),
            body,
        )
        .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Rollback: Failed to create container {name}: {e:#}");
            return;
        }
    };

    match docker
        .start_container(
            &created.id,
            Some(StartContainerOptionsBuilder::default().build()),
        )
        .await
    {
        Ok(_) => tracing::warn!(
            "Rollback successful: container {name} is running on previous image {old_image_id}"
        ),
        Err(e) => tracing::error!("Rollback: Failed to start container {name}: {e:#}"),
    }
}

/// Assembles a Docker platform string from image inspection data.
///
/// Returns `{os}/{architecture}` or `{os}/{architecture}/{variant}` when the
/// image metadata is present. Falls back to [`host_platform`] when either
/// field is missing so we always pass *something* sensible to the registry.
pub(crate) fn platform_from_inspect(
    os: Option<&str>,
    architecture: Option<&str>,
    variant: Option<&str>,
) -> String {
    match (os, architecture) {
        (Some(os), Some(arch)) => match variant.filter(|v| !v.is_empty()) {
            Some(v) => format!("{os}/{arch}/{v}"),
            None => format!("{os}/{arch}"),
        },
        _ => host_platform(),
    }
}

/// Fallback platform derived from the compiled target architecture.
///
/// Used when the local image cannot be inspected (e.g. on the very first pull
/// or if the inspect call fails unexpectedly).
pub(crate) fn host_platform() -> String {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("linux/{arch}")
}

/// Determines the platform of the locally stored image.
///
/// Prefers the image's own `os`/`architecture`/`variant` metadata so that a
/// container running an x86 image on an ARM host (via emulation) keeps pulling
/// the x86 variant rather than accidentally switching to the native arch.
async fn image_platform(docker: &Docker, image: &str) -> String {
    match docker.inspect_image(image).await {
        Ok(info) => platform_from_inspect(
            info.os.as_deref(),
            info.architecture.as_deref(),
            info.variant.as_deref(),
        ),
        Err(e) => {
            tracing::warn!(
                "Could not inspect image {image} for platform info: {e:#} — falling back to host platform"
            );
            host_platform()
        }
    }
}

/// Pulls the given image reference matching the existing image's platform,
/// respecting the configured timeout.
async fn pull_image(docker: &Docker, image: &str, timeout_secs: u64) -> Result<()> {
    let platform = image_platform(docker, image).await;
    tracing::info!("Pulling image {image} (platform: {platform})...");

    let mut stream = docker.create_image(
        Some(
            CreateImageOptionsBuilder::default()
                .from_image(image)
                .platform(&platform)
                .build(),
        ),
        None,
        None,
    );

    tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        while let Some(result) = stream.next().await {
            result.context("Image pull failed")?;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await
    .context("Image pull timed out")?
}
