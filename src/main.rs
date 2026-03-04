mod config;

use anyhow::{Context, Result};
use bollard::Docker;
use clap::Parser as _;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::parse().validate()?;
    let docker = connect_docker(cfg.host.as_deref())?;
    docker
        .version()
        .await
        .context("Docker daemon not reachable - is the socket mounted?")?;
    println!(
        "Connected to Docker daemon successfully. Version: {}",
        docker.version().await?.version.unwrap()
    );
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
