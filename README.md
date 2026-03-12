# dockguard

> A lightweight, self-contained Docker container update watcher — a spiritual successor to [containrrr/watchtower](https://github.com/containrrr/watchtower).


[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build](https://github.com/h4llow3En/dockguard/actions/workflows/ci.yml/badge.svg)](https://github.com/h4llow3En/dockguard/actions)

---

## What is dockguard?

dockguard watches your running Docker containers and automatically pulls updated images, recreates containers, and optionally cleans up stale images — on a schedule you control.

It runs either as a standalone binary (daemon or one-shot) or as a Docker container alongside your stack. Per-container behaviour is configured via Docker labels, keeping your infrastructure self-documenting.

---

## Features

- **Opt-in by default** — only containers explicitly labelled `dockguard.enable=true` are touched
- **Per-container configuration** via `dockguard.*` Docker labels (schedule, stop timeout, watch mode)
- **Flexible scheduling** — cron expression or fixed interval, set per container
- **Watch mode** — log available updates without applying them, per container
- **Remote Docker hosts** — connect via Unix socket, TCP, or HTTPS

---

## Installation

### Docker Compose (recommended)

```yaml
services:
  dockguard:
    image: ghcr.io/h4llow3en/dockguard:latest
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      GUARD_LOG_LEVEL: info
      GUARD_SELF_UPDATE: "true"
    restart: unless-stopped
```

### Pre-built binaries

Download the latest release for your platform from the [GitHub Releases page](https://github.com/h4llow3En/dockguard/releases):

| Platform | Archive |
|---|---|
| Linux x86\_64 | `dockguard-x86_64-unknown-linux-gnu.tar.gz` |
| Linux aarch64 | `dockguard-aarch64-unknown-linux-gnu.tar.gz` |
| macOS x86\_64 | `dockguard-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `dockguard-aarch64-apple-darwin.tar.gz` |

```bash
tar xzf dockguard-<target>.tar.gz
sudo mv dockguard /usr/local/bin/
```

### Debian / Ubuntu (.deb)

```bash
# Download the .deb from the releases page, then:
sudo dpkg -i dockguard_<version>_amd64.deb
```

### RHEL / Fedora / openSUSE (.rpm)

```bash
# Download the .rpm from the releases page, then:
sudo rpm -i dockguard-<version>.x86_64.rpm
# or with dnf:
sudo dnf install dockguard-<version>.x86_64.rpm
```

### cargo install

```bash
cargo install dockguard
```

### Build from source

```bash
git clone https://github.com/h4llow3En/dockguard
cd dockguard
cargo build --release
./target/release/dockguard --help
```

---

## Configuration

Most options can be set as CLI arguments or environment variables.

| Flag | Env var | Default | Description |
|---|---|---|---|
| `--clean` | `GUARD_CLEAN` | `false` | Remove old images after a successful update |
| `--host` | `DOCKER_HOST` | *(local)* | Docker host URI. When absent, `connect_with_local_defaults()` is used |
| `--enable` | `GUARD_ENABLE` | `true` | `true` = opt-in (only labelled containers), `false` = opt-out (all containers unless excluded) |
| `--log-level` | `GUARD_LOG_LEVEL` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `--pull-timeout` | `GUARD_PULL_TIMEOUT` | `300` | Seconds before an image pull is aborted |
| `--once` | N/A | `false` | Run a single update pass and exit instead of running as a daemon |
| N/A | `GUARD_SELF_UPDATE` | `false` | Also update dockguard's own container (no-op when running as a binary) |

### Docker host formats

| Format | Example |
|---|---|
| Unix socket | `unix:///var/run/docker.sock` |
| TCP (plain) | `tcp://192.168.1.10:2375` |
| TCP (TLS) | `https://192.168.1.10:2376` |
| Local defaults | *(omit `--host`)* |

---

## Container labels

dockguard is configured per container via Docker labels with the `dockguard.` prefix.

```yaml
# docker-compose.yml example
services:
  myapp:
    image: myrepo/myapp:latest
    labels:
      dockguard.enable: "true"
      dockguard.schedule: "0 3 * * *"   # update nightly at 03:00
      dockguard.watch: "false"
      dockguard.stop-timeout: "30"
```

| Label | Type | Default | Description |
|---|---|---|---|
| `dockguard.enable` | bool | *(see `--enable`)* | `true` = opt-in, `false` = exclude this container |
| `dockguard.schedule` | cron | — | Cron expression for updates. Mutually exclusive with `dockguard.interval` |
| `dockguard.interval` | integer (s) | `86400` | Update interval in seconds. Mutually exclusive with `dockguard.schedule` |
| `dockguard.stop-timeout` | integer (s) | `10` | Seconds to wait for graceful shutdown before SIGKILL |
| `dockguard.watch` | bool | `false` | Log available updates without applying them |

**Boolean labels** accept: `true`, `1`, `yes` / `false`, `0`, `no` (case-insensitive).

---

## License

MIT — see [LICENSE](LICENSE).
