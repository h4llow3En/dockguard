# Changelog

## [v0.1.0](https://github.com/h4llow3En/dockguard/releases/tag/v0.1.0) (2026-03-17)

### Features

* pull image matching the platform of the running container to preserve architecture on update
([4984745](https://github.com/h4llow3En/dockguard/commit/49847450e7c97058661e7d395d42d995437f8692))

* acquire exclusive update gate during self-update to block concurrent container updates
([4975e42](https://github.com/h4llow3En/dockguard/commit/4975e42ab7bd073d84309d32f6b066ca23e62587))

* implement full update lifecycle: pull → stop → remove → recreate → start with automatic rollback on health-check failure
([3821961](https://github.com/h4llow3En/dockguard/commit/3821961303570fd4e2603dbc50cd02c68c971ed6))

* detect own container via cgroup and include it in the update cycle when `GUARD_SELF_UPDATE=true`
([ff418d8](https://github.com/h4llow3En/dockguard/commit/ff418d88394480ec4d214a4aa659d592f995c928))

* add per-container scheduler driven by `dockguard.interval` or `dockguard.schedule` (cron) with digest-based update detection
([3d18839](https://github.com/h4llow3En/dockguard/commit/3d188390c70c3f5eb857c6d71db666c5c6202a5a))

* add internal health check endpoint for Docker `HEALTHCHECK` instruction
([4b7b3fb](https://github.com/h4llow3En/dockguard/commit/4b7b3fb57c27663d67f9fd51ce6af49509b7f9a5))

* watch running containers and react to `start` / `die` / `destroy` Docker events
([8559204](https://github.com/h4llow3En/dockguard/commit/85592041d3c675e017205da4cdff2e0640c06bb3))

* configure management via container labels (`dockguard.enable`, `dockguard.interval`, `dockguard.schedule`, `dockguard.stop-timeout`, `dockguard.watch`)
([4e62478](https://github.com/h4llow3En/dockguard/commit/4e6247886e405c24f4da748368d298724592170f))
