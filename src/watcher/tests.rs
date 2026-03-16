use super::*;
use crate::labels::UpdateTrigger;

fn labels(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// --- opt-in Modus (global_enable = true) ---

#[test]
fn opt_in_without_label_is_not_managed() {
    let result = try_build_managed(
        "id",
        "name",
        "image",
        "image_id",
        &HashMap::new(),
        true,
        false,
    );
    assert!(result.is_none());
}

#[test]
fn opt_in_with_enable_true_is_managed() {
    let l = labels(&[("dockguard.enable", "true")]);
    let result = try_build_managed(
        "abc123",
        "/my-container",
        "nginx:latest",
        "sha256:abc",
        &l,
        true,
        false,
    );
    let mc = result.expect("should be managed");
    assert_eq!(mc.id, "abc123");
    assert_eq!(mc.name, "/my-container");
    assert_eq!(mc.image, "nginx:latest");
    assert_eq!(mc.image_id, "sha256:abc");
}

#[test]
fn opt_in_with_enable_false_is_not_managed() {
    let l = labels(&[("dockguard.enable", "false")]);
    let result = try_build_managed("id", "name", "image", "image_id", &l, true, false);
    assert!(result.is_none());
}

// --- opt-out Modus (global_enable = false) ---

#[test]
fn opt_out_without_label_is_managed() {
    let result = try_build_managed(
        "id",
        "name",
        "image",
        "image_id",
        &HashMap::new(),
        false,
        false,
    );
    assert!(result.is_some());
}

#[test]
fn opt_out_with_enable_false_is_not_managed() {
    let l = labels(&[("dockguard.enable", "false")]);
    let result = try_build_managed("id", "name", "image", "image_id", &l, false, false);
    assert!(result.is_none());
}

// --- force_enable (self-update) ---

#[test]
fn force_enable_overrides_opt_in_without_label() {
    let result = try_build_managed(
        "id",
        "name",
        "image",
        "image_id",
        &HashMap::new(),
        true,
        true,
    );
    assert!(result.is_some());
}

#[test]
fn force_enable_overrides_explicit_enable_false() {
    let l = labels(&[("dockguard.enable", "false")]);
    let result = try_build_managed("id", "name", "image", "image_id", &l, true, true);
    assert!(result.is_some());
}

// --- Ungültige Labels ---

#[test]
fn invalid_enable_label_returns_none() {
    let l = labels(&[("dockguard.enable", "maybe")]);
    let result = try_build_managed("id", "name", "image", "image_id", &l, true, false);
    assert!(result.is_none());
}

#[test]
fn mutually_exclusive_schedule_and_interval_returns_none() {
    let l = labels(&[
        ("dockguard.enable", "true"),
        ("dockguard.schedule", "0 3 * * *"),
        ("dockguard.interval", "3600"),
    ]);
    let result = try_build_managed("id", "name", "image", "image_id", &l, true, false);
    assert!(result.is_none());
}

// --- Trigger-Typen ---

#[test]
fn schedule_trigger_is_preserved() {
    let l = labels(&[
        ("dockguard.enable", "true"),
        ("dockguard.schedule", "0 3 * * *"),
    ]);
    let mc = try_build_managed("id", "name", "image", "image_id", &l, true, false)
        .expect("should be managed");
    assert!(matches!(
        mc.config.update_trigger,
        UpdateTrigger::Schedule(_)
    ));
}

#[test]
fn interval_trigger_is_preserved() {
    let l = labels(&[("dockguard.enable", "true"), ("dockguard.interval", "3600")]);
    let mc = try_build_managed("id", "name", "image", "image_id", &l, true, false)
        .expect("should be managed");
    assert!(matches!(
        mc.config.update_trigger,
        UpdateTrigger::Interval(3600)
    ));
}

// --- watch / stop-timeout ---

#[test]
fn watch_mode_is_preserved() {
    let l = labels(&[("dockguard.enable", "true"), ("dockguard.watch", "true")]);
    let mc = try_build_managed("id", "name", "image", "image_id", &l, true, false).unwrap();
    assert!(mc.config.watch);
}

#[test]
fn stop_timeout_is_preserved() {
    let l = labels(&[
        ("dockguard.enable", "true"),
        ("dockguard.stop-timeout", "30"),
    ]);
    let mc = try_build_managed("id", "name", "image", "image_id", &l, true, false).unwrap();
    assert_eq!(mc.config.stop_timeout, 30);
}

#[test]
fn cancel_token_is_not_cancelled_initially() {
    let l = labels(&[("dockguard.enable", "true")]);
    let mc = try_build_managed("id", "name", "image", "image_id", &l, true, false).unwrap();
    assert!(!mc.cancel_token.is_cancelled());
}

#[test]
fn default_trigger_is_interval_86400() {
    let l = labels(&[("dockguard.enable", "true")]);
    let mc = try_build_managed("id", "name", "image", "image_id", &l, true, false)
        .expect("should be managed");
    assert!(matches!(
        mc.config.update_trigger,
        UpdateTrigger::Interval(86400)
    ));
}
