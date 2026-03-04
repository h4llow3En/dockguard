use super::*;

fn base_config() -> Config {
    Config {
        clean: false,
        host: None,
        schedule: None,
        intervall: None,
        watch: false,
        log_level: "info".to_string(),
        pull_timeout: 300,
        stop_timeout: 10,
        once: false,
    }
}

#[test]
fn default_trigger_is_interval_86400() {
    let cfg = base_config().validate().unwrap();
    assert!(matches!(cfg.update_trigger, UpdateTrigger::Interval(86400)));
}

#[test]
fn explicit_intervall_is_used() {
    let cfg = Config { intervall: Some(3600), ..base_config() }.validate().unwrap();
    assert!(matches!(cfg.update_trigger, UpdateTrigger::Interval(3600)));
}

#[test]
fn schedule_is_used() {
    let cfg = Config {
        schedule: Some("0 3 * * *".to_string()),
        ..base_config()
    }
    .validate()
    .unwrap();
    assert!(matches!(cfg.update_trigger, UpdateTrigger::Schedule(_)));
}

#[test]
fn both_set_returns_error() {
    let cfg = Config {
        schedule: Some("0 3 * * *".to_string()),
        intervall: Some(3600),
        ..base_config()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn invalid_cron_returns_error() {
    let cfg = Config {
        schedule: Some("not-a-cron".to_string()),
        ..base_config()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn zero_pull_timeout_returns_error() {
    let cfg = Config { pull_timeout: 0, ..base_config() };
    assert!(cfg.validate().is_err());
}

#[test]
fn invalid_host_scheme_returns_error() {
    let cfg = Config {
        host: Some("ftp://example.com".to_string()),
        ..base_config()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn valid_host_scheme_passes() {
    for scheme in ["unix:///var/run/docker.sock", "tcp://host:2375", "https://host:2376"] {
        let cfg = Config {
            host: Some(scheme.to_string()),
            ..base_config()
        };
        assert!(cfg.validate().is_ok(), "scheme should be valid: {scheme}");
    }
}
