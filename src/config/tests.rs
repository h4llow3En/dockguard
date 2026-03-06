use super::*;

fn base_config() -> Config {
    Config {
        clean: false,
        host: None,
        enable: true,
        log_level: "info".to_string(),
        pull_timeout: 300,
        once: false,
    }
}

#[test]
fn default_config_validates_successfully() {
    assert!(base_config().validate().is_ok());
}

#[test]
fn opt_in_mode_is_preserved() {
    let cfg = Config {
        enable: true,
        ..base_config()
    }
    .validate()
    .unwrap();
    assert!(cfg.enable);
}

#[test]
fn opt_out_mode_is_preserved() {
    let cfg = Config {
        enable: false,
        ..base_config()
    }
    .validate()
    .unwrap();
    assert!(!cfg.enable);
}

#[test]
fn zero_pull_timeout_returns_error() {
    let cfg = Config {
        pull_timeout: 0,
        ..base_config()
    };
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
    for scheme in [
        "unix:///var/run/docker.sock",
        "tcp://host:2375",
        "https://host:2376",
    ] {
        let cfg = Config {
            host: Some(scheme.to_string()),
            ..base_config()
        };
        assert!(cfg.validate().is_ok(), "scheme should be valid: {scheme}");
    }
}
