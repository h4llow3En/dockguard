use super::*;
use crate::labels::{ResolvedContainerConfig, UpdateTrigger};
use std::time::Duration;
use tokio::time;
use tokio_util::sync::CancellationToken;

fn make_container(trigger: UpdateTrigger) -> ManagedContainer {
    ManagedContainer {
        id: "test-id".to_string(),
        name: "test-container".to_string(),
        image: "nginx:latest".to_string(),
        image_id: "sha256:abc".to_string(),
        config: ResolvedContainerConfig {
            enabled: true,
            update_trigger: trigger,
            stop_timeout: 10,
            watch: false,
        },
        cancel_token: CancellationToken::new(),
    }
}

// --- UpdateStatus ---

#[test]
fn update_status_up_to_date_variant() {
    assert!(matches!(UpdateStatus::UpToDate, UpdateStatus::UpToDate));
}

#[test]
fn update_status_update_available_carries_digests() {
    let s = UpdateStatus::UpdateAvailable {
        local: "sha256:old".to_string(),
        remote: "sha256:new".to_string(),
    };
    if let UpdateStatus::UpdateAvailable { local, remote } = s {
        assert_eq!(local, "sha256:old");
        assert_eq!(remote, "sha256:new");
    } else {
        panic!("wrong variant");
    }
}

// --- trigger_wait ---

#[tokio::test]
async fn trigger_wait_interval_completes_after_advance() {
    time::pause();
    let trigger = UpdateTrigger::Interval(60);
    let handle = tokio::spawn(async move {
        trigger_wait(&trigger).await;
    });
    time::advance(Duration::from_secs(60)).await;
    handle.await.unwrap();
}

#[tokio::test]
async fn trigger_wait_invalid_cron_falls_back_to_1h() {
    time::pause();
    let trigger = UpdateTrigger::Schedule("not-a-valid-cron".to_string());
    let handle = tokio::spawn(async move {
        trigger_wait(&trigger).await;
    });
    time::advance(Duration::from_secs(3600)).await;
    handle.await.unwrap();
}

#[tokio::test]
async fn trigger_wait_valid_schedule_completes() {
    time::pause();
    // 6-field cron (sec min hour dom month dow) — every second.
    // Next fire is ≤ 1 s away; the unwrap_or fallback is 60 s.
    // Advance 61 s to cover both cases safely.
    let trigger = UpdateTrigger::Schedule("* * * * * *".to_string());
    let handle = tokio::spawn(async move {
        trigger_wait(&trigger).await;
    });
    time::advance(Duration::from_secs(61)).await;
    handle.await.unwrap();
}

// --- run ---

#[tokio::test]
async fn run_exits_when_token_pre_cancelled() {
    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let container = make_container(UpdateTrigger::Interval(86400));
    container.cancel_token.cancel();
    // Should return immediately without sleeping the full interval
    run(docker, container).await;
}

#[tokio::test]
async fn run_exits_when_token_cancelled_after_spawn() {
    time::pause();
    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let container = make_container(UpdateTrigger::Interval(86400));
    let token = container.cancel_token.clone();

    let handle = tokio::spawn(run(docker, container));
    token.cancel();
    // Yield to the runtime so the spawned task can poll and see the cancellation
    time::advance(Duration::from_millis(1)).await;
    handle.await.unwrap();
}
