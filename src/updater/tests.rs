use super::*;
use bollard::models::{ContainerState, ContainerStateStatusEnum, Health, HealthStatusEnum};
use std::time::Duration;

// --- host_platform (fallback) ---

#[test]
fn host_platform_starts_with_linux() {
    assert!(host_platform().starts_with("linux/"));
}

#[test]
fn host_platform_has_non_empty_arch() {
    let p = host_platform();
    let arch = p.strip_prefix("linux/").unwrap();
    assert!(!arch.is_empty());
}

#[test]
fn host_platform_known_arch_is_mapped() {
    let p = host_platform();
    match std::env::consts::ARCH {
        "x86_64" => assert_eq!(p, "linux/amd64"),
        "aarch64" => assert_eq!(p, "linux/arm64"),
        _ => {}
    }
}

// --- platform_from_inspect ---

#[test]
fn platform_from_inspect_os_and_arch() {
    assert_eq!(
        platform_from_inspect(Some("linux"), Some("amd64"), None),
        "linux/amd64"
    );
}

#[test]
fn platform_from_inspect_with_variant() {
    assert_eq!(
        platform_from_inspect(Some("linux"), Some("arm"), Some("v7")),
        "linux/arm/v7"
    );
}

#[test]
fn platform_from_inspect_empty_variant_is_ignored() {
    assert_eq!(
        platform_from_inspect(Some("linux"), Some("arm64"), Some("")),
        "linux/arm64"
    );
}

#[test]
fn platform_from_inspect_missing_arch_falls_back_to_host() {
    let result = platform_from_inspect(Some("linux"), None, None);
    assert_eq!(result, host_platform());
}

#[test]
fn platform_from_inspect_missing_os_falls_back_to_host() {
    let result = platform_from_inspect(None, Some("amd64"), None);
    assert_eq!(result, host_platform());
}

#[test]
fn platform_from_inspect_x86_image_on_any_host() {
    // Simulates an x86 image running on an ARM host via emulation.
    // The pulled image must stay x86.
    assert_eq!(
        platform_from_inspect(Some("linux"), Some("amd64"), None),
        "linux/amd64"
    );
}

fn state(status: ContainerStateStatusEnum, health: Option<HealthStatusEnum>) -> ContainerState {
    ContainerState {
        status: Some(status),
        health: health.map(|s| Health {
            status: Some(s),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn assert_healthy(s: &ContainerState) {
    assert!(matches!(
        classify_state(s),
        Some(HealthWaitOutcome::Healthy)
    ));
}

fn assert_failed(s: &ContainerState) {
    assert!(matches!(classify_state(s), Some(HealthWaitOutcome::Failed)));
}

fn assert_pending(s: &ContainerState) {
    assert!(classify_state(s).is_none());
}

#[test]
fn exited_without_health_is_failed() {
    assert_failed(&state(ContainerStateStatusEnum::EXITED, None));
}

#[test]
fn dead_without_health_is_failed() {
    assert_failed(&state(ContainerStateStatusEnum::DEAD, None));
}

#[test]
fn exited_with_healthy_status_is_still_failed() {
    // EXITED takes priority over the health field.
    assert_failed(&state(
        ContainerStateStatusEnum::EXITED,
        Some(HealthStatusEnum::HEALTHY),
    ));
}

#[test]
fn running_without_health_field_is_healthy() {
    assert_healthy(&state(ContainerStateStatusEnum::RUNNING, None));
}

#[test]
fn created_without_health_field_is_pending() {
    assert_pending(&state(ContainerStateStatusEnum::CREATED, None));
}

#[test]
fn restarting_without_health_field_is_pending() {
    assert_pending(&state(ContainerStateStatusEnum::RESTARTING, None));
}

#[test]
fn running_with_health_none_is_healthy() {
    // Docker reports status "none" when the image has no HEALTHCHECK instruction.
    assert_healthy(&state(
        ContainerStateStatusEnum::RUNNING,
        Some(HealthStatusEnum::NONE),
    ));
}

#[test]
fn running_with_health_healthy_is_healthy() {
    assert_healthy(&state(
        ContainerStateStatusEnum::RUNNING,
        Some(HealthStatusEnum::HEALTHY),
    ));
}

#[test]
fn running_with_health_unhealthy_is_failed() {
    assert_failed(&state(
        ContainerStateStatusEnum::RUNNING,
        Some(HealthStatusEnum::UNHEALTHY),
    ));
}

#[test]
fn running_with_health_starting_is_pending() {
    assert_pending(&state(
        ContainerStateStatusEnum::RUNNING,
        Some(HealthStatusEnum::STARTING),
    ));
}

#[test]
fn restarting_with_health_starting_is_pending() {
    assert_pending(&state(
        ContainerStateStatusEnum::RESTARTING,
        Some(HealthStatusEnum::STARTING),
    ));
}

// ── wait_healthy: Docker not required ────────────────────────────────────────

/// A container ID that can never exist returns Failed immediately because
/// the inspect call either gets a 404 (Docker running) or a connection error
/// (Docker not running) — both are treated as Err(_) → Failed.
#[tokio::test]
async fn wait_healthy_returns_failed_for_nonexistent_container() {
    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let outcome = wait_healthy(
        &docker,
        "nonexistent-container-id-00000000",
        Duration::from_secs(5),
    )
    .await;
    assert!(matches!(outcome, HealthWaitOutcome::Failed));
}
