mod config;
mod labels;
mod logging;
mod self_container;

use anyhow::{Context, Result};
use bollard::Docker;
use clap::Parser as _;
use config::{Config, ValidatedConfig};

#[tokio::main]
async fn main() {
    let cfg = match Config::parse().validate() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    };

    if let Err(e) = logging::init(cfg.log_level) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }

    if let Err(e) = run(cfg).await {
        tracing::error!("Application error: {e:#}");
        std::process::exit(1);
    }
}

async fn run(cfg: ValidatedConfig) -> Result<()> {
    let docker = connect_docker(cfg.host.as_deref())?;
    docker
        .version()
        .await
        .context("Docker daemon not reachable - is the socket mounted?")?;

    let version = docker.version().await?.version.unwrap_or_default();
    tracing::info!("Connected to Docker daemon (version {version})");
    Ok(())
}

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
