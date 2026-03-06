use std::path::Path;

use bollard::Docker;
use bollard::models::ContainerInspectResponse;

/// Attempts to detect the full container ID of the current process when running inside Docker.
/// Returns `None` if dockguard is not running in a container or if detection fails.
///
/// Detection strategy:
/// 1. `/.dockerenv` must exist (Docker creates this in every container).
/// 2. Parse `/proc/self/cgroup` to extract the container ID (works on cgroup v1 and v2).
/// 3. Fall back to the `HOSTNAME` env var, which Docker sets to the short container ID.
pub fn detect_own_container_id() -> Option<String> {
    if !Path::new("/.dockerenv").exists() {
        return None;
    }

    if let Ok(cgroup) = std::fs::read_to_string("/proc/self/cgroup")
        && let Some(id) = extract_from_cgroup(&cgroup)
    {
        return Some(id);
    }

    // Fallback: Docker sets HOSTNAME to the container's short ID by default.
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| looks_like_container_id(h))
}

/// Parses `/proc/self/cgroup` content and returns the first container ID found.
pub(crate) fn extract_from_cgroup(content: &str) -> Option<String> {
    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        parts.next()?; // hierarchy-id
        parts.next()?; // subsystem list
        let path = parts.next()?;

        if let Some(id) = parse_container_id_from_path(path) {
            return Some(id);
        }
    }
    None
}

/// Extracts a Docker container ID from a cgroup path segment.
///
/// Handles:
/// - cgroup v1: `/docker/<id>` or `/docker/<id>/...`
/// - cgroup v2: `/system.slice/docker-<id>.scope`
pub(crate) fn parse_container_id_from_path(path: &str) -> Option<String> {
    // cgroup v1: path starts with /docker/<id>
    if let Some(rest) = path.strip_prefix("/docker/") {
        let id = rest.split('/').next().unwrap_or("");
        if looks_like_container_id(id) {
            return Some(id.to_string());
        }
    }

    // cgroup v2: docker-<id>.scope somewhere in the path
    if let Some(after_prefix) = path.split_once("docker-").map(|(_, r)| r)
        && let Some(id) = after_prefix.split_once(".scope").map(|(id, _)| id)
        && looks_like_container_id(id)
    {
        return Some(id.to_string());
    }

    None
}

/// A string looks like a Docker container ID if it is at least 12 hex characters.
fn looks_like_container_id(s: &str) -> bool {
    s.len() >= 12 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Resolves dockguard's own container via the Docker API.
///
/// Returns `None` if dockguard is not running inside Docker, or if the
/// container cannot be found (e.g. the socket is not mounted with the
/// container's own ID visible). Inspect errors are logged as warnings so
/// that a misconfigured self-update never crashes the daemon.
pub async fn resolve_own_container(docker: &Docker) -> Option<ContainerInspectResponse> {
    let id = detect_own_container_id()?;

    match docker.inspect_container(&id, None).await {
        Ok(info) => Some(info),
        Err(e) => {
            tracing::warn!("Could not inspect own container (id={id}): {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests;
