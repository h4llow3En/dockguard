use super::*;
use std::collections::HashMap;

fn labels(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// --- from_map ---

#[test]
fn empty_map_gives_all_none() {
    let cl = ContainerLabels::from_map(&HashMap::new()).unwrap();
    assert!(cl.enable.is_none());
    assert!(cl.schedule.is_none());
    assert!(cl.interval.is_none());
    assert!(cl.stop_timeout.is_none());
    assert!(cl.watch.is_none());
    assert!(cl.unknown_labels.is_empty());
}

#[test]
fn unknown_dockguard_labels_are_collected() {
    let cl = ContainerLabels::from_map(&labels(&[
        (LABEL_ENABLE, "true"),
        ("dockguard.foo", "bar"),
        ("dockguard.unknown-thing", "42"),
        ("traefik.enable", "true"), // foreign prefix – must not appear
    ]))
    .unwrap();
    assert_eq!(cl.unknown_labels.len(), 2);
    assert!(cl.unknown_labels.contains(&"dockguard.foo".to_string()));
    assert!(
        cl.unknown_labels
            .contains(&"dockguard.unknown-thing".to_string())
    );
}

#[test]
fn known_labels_are_not_in_unknown() {
    let cl = ContainerLabels::from_map(&labels(&[
        (LABEL_ENABLE, "true"),
        (LABEL_SCHEDULE, "0 3 * * *"),
        (LABEL_INTERVAL, "3600"),
        (LABEL_STOP_TIMEOUT, "30"),
        (LABEL_WATCH, "false"),
    ]))
    .unwrap();
    assert!(cl.unknown_labels.is_empty());
}

#[test]
fn parses_all_labels() {
    let cl = ContainerLabels::from_map(&labels(&[
        (LABEL_ENABLE, "true"),
        (LABEL_SCHEDULE, "0 3 * * *"),
        (LABEL_STOP_TIMEOUT, "30"),
        (LABEL_WATCH, "false"),
    ]))
    .unwrap();
    assert_eq!(cl.enable, Some(true));
    assert_eq!(cl.schedule.as_deref(), Some("0 3 * * *"));
    assert_eq!(cl.stop_timeout, Some(30));
    assert_eq!(cl.watch, Some(false));
}

#[test]
fn bool_accepts_variants() {
    for truthy in ["true", "1", "yes"] {
        let cl = ContainerLabels::from_map(&labels(&[(LABEL_ENABLE, truthy)])).unwrap();
        assert_eq!(cl.enable, Some(true), "expected true for '{truthy}'");
    }
    for falsy in ["false", "0", "no"] {
        let cl = ContainerLabels::from_map(&labels(&[(LABEL_ENABLE, falsy)])).unwrap();
        assert_eq!(cl.enable, Some(false), "expected false for '{falsy}'");
    }
}

#[test]
fn invalid_bool_returns_error() {
    assert!(ContainerLabels::from_map(&labels(&[(LABEL_ENABLE, "maybe")])).is_err());
}

#[test]
fn invalid_interval_returns_error() {
    assert!(ContainerLabels::from_map(&labels(&[(LABEL_INTERVAL, "not-a-number")])).is_err());
}

#[test]
fn invalid_stop_timeout_returns_error() {
    assert!(ContainerLabels::from_map(&labels(&[(LABEL_STOP_TIMEOUT, "-5")])).is_err());
}

// --- resolve ---

#[test]
fn opt_in_no_label_defaults_to_disabled() {
    let resolved = ContainerLabels::from_map(&HashMap::new())
        .unwrap()
        .resolve(true)
        .unwrap();
    assert!(!resolved.enabled);
}

#[test]
fn opt_out_no_label_defaults_to_enabled() {
    let resolved = ContainerLabels::from_map(&HashMap::new())
        .unwrap()
        .resolve(false)
        .unwrap();
    assert!(resolved.enabled);
}

#[test]
fn explicit_enable_overrides_opt_in() {
    let resolved = ContainerLabels::from_map(&labels(&[(LABEL_ENABLE, "true")]))
        .unwrap()
        .resolve(true)
        .unwrap();
    assert!(resolved.enabled);
}

#[test]
fn explicit_disable_overrides_opt_out() {
    let resolved = ContainerLabels::from_map(&labels(&[(LABEL_ENABLE, "false")]))
        .unwrap()
        .resolve(false)
        .unwrap();
    assert!(!resolved.enabled);
}

#[test]
fn default_trigger_is_interval_86400() {
    let resolved = ContainerLabels::from_map(&HashMap::new())
        .unwrap()
        .resolve(false)
        .unwrap();
    assert_eq!(resolved.update_trigger, UpdateTrigger::Interval(86400));
}

#[test]
fn interval_label_is_used() {
    let resolved = ContainerLabels::from_map(&labels(&[(LABEL_INTERVAL, "3600")]))
        .unwrap()
        .resolve(false)
        .unwrap();
    assert_eq!(resolved.update_trigger, UpdateTrigger::Interval(3600));
}

#[test]
fn schedule_label_is_used() {
    let resolved = ContainerLabels::from_map(&labels(&[(LABEL_SCHEDULE, "0 3 * * *")]))
        .unwrap()
        .resolve(false)
        .unwrap();
    assert!(matches!(
        resolved.update_trigger,
        UpdateTrigger::Schedule(_)
    ));
}

#[test]
fn both_schedule_and_interval_returns_error() {
    let result = ContainerLabels::from_map(&labels(&[
        (LABEL_SCHEDULE, "0 3 * * *"),
        (LABEL_INTERVAL, "3600"),
    ]))
    .unwrap()
    .resolve(false);
    assert!(result.is_err());
}

#[test]
fn invalid_cron_returns_error() {
    let result = ContainerLabels::from_map(&labels(&[(LABEL_SCHEDULE, "not-a-cron")]))
        .unwrap()
        .resolve(false);
    assert!(result.is_err());
}

#[test]
fn zero_interval_returns_error() {
    let result = ContainerLabels::from_map(&labels(&[(LABEL_INTERVAL, "0")]))
        .unwrap()
        .resolve(false);
    assert!(result.is_err());
}

#[test]
fn default_stop_timeout_is_10() {
    let resolved = ContainerLabels::from_map(&HashMap::new())
        .unwrap()
        .resolve(false)
        .unwrap();
    assert_eq!(resolved.stop_timeout, 10);
}

#[test]
fn custom_stop_timeout_is_used() {
    let resolved = ContainerLabels::from_map(&labels(&[(LABEL_STOP_TIMEOUT, "30")]))
        .unwrap()
        .resolve(false)
        .unwrap();
    assert_eq!(resolved.stop_timeout, 30);
}

#[test]
fn default_watch_is_false() {
    let resolved = ContainerLabels::from_map(&HashMap::new())
        .unwrap()
        .resolve(false)
        .unwrap();
    assert!(!resolved.watch);
}

#[test]
fn watch_label_is_used() {
    let resolved = ContainerLabels::from_map(&labels(&[(LABEL_WATCH, "true")]))
        .unwrap()
        .resolve(false)
        .unwrap();
    assert!(resolved.watch);
}
