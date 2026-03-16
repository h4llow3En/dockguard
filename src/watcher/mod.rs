use crate::config::ValidatedConfig;
use crate::labels::{ContainerLabels, ResolvedContainerConfig};
use crate::scheduler;
use anyhow::Result;
use bollard::Docker;
use bollard::query_parameters::{EventsOptions, ListContainersOptions};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct ManagedContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub config: ResolvedContainerConfig,
    pub cancel_token: CancellationToken,
}

pub type ManagedContainers = Arc<RwLock<HashMap<String, ManagedContainer>>>;

/// Shared gate that serialises self-updates against all other container updates.
///
/// - Regular updates acquire a **read** guard (many can run in parallel).
/// - Self-updates acquire the **write** guard (exclusive: waits for all running
///   updates to finish, then prevents new ones from starting).
pub type UpdateGate = Arc<RwLock<()>>;

/// Try building `ManagedContainer` from container metadata.
/// Returns `None` if the container should not be managed
/// or if the labels are invalid (errors are logged as `warn`).
pub(crate) fn try_build_managed(
    id: &str,
    name: &str,
    image: &str,
    image_id: &str,
    labels: &HashMap<String, String>,
    global_enable: bool,
    force_enable: bool,
) -> Option<ManagedContainer> {
    let parsed = ContainerLabels::from_map(labels)
        .map_err(|e| tracing::warn!("Container {name}: invalid labels: {e:#}"))
        .ok()?;
    if !parsed.unknown_labels.is_empty() {
        tracing::info!(
            "Container {name}: ignoring unknown labels: {}",
            parsed.unknown_labels.join(", ")
        );
    }
    let resolved = parsed
        .resolve(global_enable)
        .map_err(|e| tracing::warn!("Container {name}: invalid config: {e:#}"))
        .ok()?;
    if !resolved.enabled && !force_enable {
        return None;
    }
    Some(ManagedContainer {
        id: id.to_string(),
        name: name.to_string(),
        image: image.to_string(),
        image_id: image_id.to_string(),
        config: resolved,
        cancel_token: CancellationToken::new(),
    })
}

#[cfg(test)]
mod tests;

/// Builds up a Map of containers that should be managed by dockguard
/// and then listens for Docker events to keep it updated.
pub async fn watch(
    docker: &Docker,
    cfg: Arc<ValidatedConfig>,
    managed: ManagedContainers,
    own_container_id: Option<String>,
    gate: UpdateGate,
) -> Result<()> {
    let mut event_stream = docker.events(Some(EventsOptions {
        filters: Some(HashMap::from([(
            "type".to_string(),
            vec!["container".to_string()],
        )])),
        ..Default::default()
    }));

    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: false,
            ..Default::default()
        }))
        .await?;

    tracing::info!("Startup: Found {} running containers", containers.len());

    let empty = HashMap::new();
    for container in &containers {
        let name = container
            .names
            .as_ref()
            .and_then(|n| n.first())
            .map(|s| s.trim_start_matches('/'))
            .unwrap_or("<unknown>");
        let image = container.image.as_deref().unwrap_or("<unknown>");
        let id = container.id.as_deref().unwrap_or_default();
        let image_id = container.image_id.as_deref().unwrap_or_default();
        let label_map = container.labels.as_ref().unwrap_or(&empty);

        let is_own = own_container_id.as_deref() == Some(id);
        let force_enable = is_own && cfg.self_update;
        if let Some(mc) = try_build_managed(
            id,
            name,
            image,
            image_id,
            label_map,
            cfg.enable,
            force_enable,
        ) {
            if is_own && !cfg.self_update {
                tracing::warn!(
                    "dockguard Container has management label set but GUARD_SELF_UPDATE is not set — own container will not be managed. Set GUARD_SELF_UPDATE=true to enable self-updates."
                );
            } else {
                tracing::info!(
                    "Managing container {name} (image: {image}) with trigger: {:?}",
                    mc.config.update_trigger
                );
                tokio::spawn(scheduler::run(
                    docker.clone(),
                    mc.clone(),
                    Arc::clone(&cfg),
                    Arc::clone(&gate),
                    force_enable,
                ));
                managed.write().await.insert(mc.id.clone(), mc);
            }
        } else {
            tracing::debug!("Container {name} (image: {image}) is not enabled for management");
        }
    }

    tracing::info!("Managing {} containers", managed.read().await.len());
    tracing::info!("Listening for container events...");

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(e) => {
                let action = e.action.as_deref().unwrap_or("");
                let id = e
                    .actor
                    .as_ref()
                    .and_then(|actor| actor.id.as_deref())
                    .unwrap_or("<unknown>");

                tracing::debug!(action = &action, id = &id, "Received event");

                match action {
                    "start" => match docker.inspect_container(id, None).await {
                        Ok(info) => {
                            let name = info
                                .name
                                .as_deref()
                                .unwrap_or("<unknown>")
                                .trim_start_matches('/');
                            let image = info
                                .config
                                .as_ref()
                                .and_then(|c| c.image.as_deref())
                                .unwrap_or("<unknown>");
                            let image_id = info.image.as_deref().unwrap_or_default();
                            let labels = info
                                .config
                                .as_ref()
                                .and_then(|c| c.labels.as_ref())
                                .unwrap_or(&empty);

                            if let Some(mc) = try_build_managed(
                                id, name, image, image_id, labels, cfg.enable, false,
                            ) {
                                tracing::info!(
                                    "New container {name} (image: {image}) with trigger: {:?}",
                                    mc.config.update_trigger
                                );
                                tokio::spawn(scheduler::run(
                                    docker.clone(),
                                    mc.clone(),
                                    Arc::clone(&cfg),
                                    Arc::clone(&gate),
                                    false,
                                ));
                                managed.write().await.insert(mc.id.clone(), mc);
                            } else {
                                tracing::debug!("New container {name} - not managed");
                            }
                        }
                        Err(err) => tracing::error!("Failed to inspect container {id}: {err:#}"),
                    },
                    "die" | "destroy" => {
                        if let Some(mc) = managed.write().await.remove(id) {
                            mc.cancel_token.cancel();
                            tracing::info!(
                                "Container {} stopped/destroyed - no longer managing",
                                mc.name
                            );
                        }
                    }

                    _ => {}
                }
            }
            Err(err) => {
                tracing::error!("Error receiving Docker events: {err:#}");
            }
        }
    }

    Ok(())
}
