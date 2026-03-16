use super::*;
use bollard::models::{ContainerState, ContainerStateStatusEnum, Health, HealthStatusEnum};
use std::time::Duration;

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
