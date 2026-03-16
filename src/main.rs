mod config;
mod labels;
mod logging;
mod scheduler;
mod self_container;
mod updater;
mod watcher;

use anyhow::{Context, Result};
use bollard::Docker;
use clap::Parser as _;
use config::{Config, ValidatedConfig};
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let cfg = match Config::parse().validate() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    };

    if cfg.healthcheck {
        if let Ok(mut stream) = std::net::TcpStream::connect("127.0.0.1:27748") {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            let mut buf = [0; 2];
            if let Ok(2) = stream.read(&mut buf)
                && &buf == b"ok"
            {
                std::process::exit(0);
            }
        }
        std::process::exit(1);
    }

    if let Err(e) = logging::init(cfg.log_level) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }

    if let Err(e) = run(cfg).await {
        tracing::error!("Application error: {e:#}");
        std::process::exit(1);
    }
}

/// Main application logic after configuration and logging are set up.
async fn run(cfg: Arc<ValidatedConfig>) -> Result<()> {
    let managed = Arc::new(RwLock::new(HashMap::new()));
    let docker = connect_docker(cfg.host.as_deref())?;
    docker
        .version()
        .await
        .context("Docker daemon not reachable - is the socket mounted?")?;

    let version = docker.version().await?.version.unwrap_or_default();
    tracing::info!("Connected to Docker daemon (version {version})");

    let docker_watch = docker.clone();
    let cfg_watch = Arc::clone(&cfg);
    let managed_watch = Arc::clone(&managed);

    let own_container_id: Option<String> = if cfg.self_update {
        match self_container::resolve_own_container(&docker).await {
            Some(info) => {
                let name = info.name.as_deref().unwrap_or("<unknown>");
                let image = info
                    .config
                    .as_ref()
                    .and_then(|c| c.image.as_deref())
                    .unwrap_or("<unknown>");
                tracing::info!("Self-update enabled — own container: {name} (image: {image})");
                info.id
            }
            None => {
                tracing::warn!(
                    "Self-update enabled but dockguard does not appear to be running inside Docker — skipping self-update"
                );
                None
            }
        }
    } else {
        // Detect own container ID even without self-update so the watcher can warn
        // if dockguard.enable is set on the own container without GUARD_SELF_UPDATE.
        self_container::detect_own_container_id()
    };

    let docker_health = docker.clone();
    tokio::spawn(async move {
        match TcpListener::bind("127.0.0.1:27748").await {
            Ok(listener) => loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let response = if docker_health.ping().await.is_ok() {
                        b"ok"
                    } else {
                        b"no"
                    };
                    let _ = stream.write_all(response).await;
                }
            },
            Err(e) => {
                tracing::error!("Failed to start healthcheck listener: {e:#}");
            }
        }
    });

    let watcher = tokio::spawn(async move {
        if let Err(e) =
            watcher::watch(&docker_watch, cfg_watch, managed_watch, own_container_id).await
        {
            tracing::error!("Container watch error: {e:#}");
        }
    });

    let _ = watcher.await;

    Ok(())
}

/// Connects to the Docker daemon using the specified host URI or local defaults.
fn connect_docker(host: Option<&str>) -> Result<Docker> {
    match host {
        None => Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon using local defaults"),
        Some(uri) if uri.starts_with("unix://") => {
            let path = uri.trim_start_matches("unix://");
            Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION)
                .context("Failed to connect to Docker daemon via Unix socket")
        }
        Some(uri) => Docker::connect_with_http(uri, 120, bollard::API_DEFAULT_VERSION)
            .context("Failed to connect to Docker daemon via HTTP"),
    }
}
