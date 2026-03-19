# Changelog

### [v0.1.6](https://github.com/h4llow3En/dockguard/compare/v0.1.5...v0.1.6) (2026-03-19)

#### Fixes

* add platform independent docker pull if platform specific fails
([3c78d97](https://github.com/h4llow3En/dockguard/commit/3c78d97b978193170336c00ab11ba758eb22e2c6))

### [v0.1.5](https://github.com/h4llow3En/dockguard/compare/v0.1.4...v0.1.5) (2026-03-18)

#### Fixes

* change rwlock for selfupdate to queue to update container gradually
([440bd22](https://github.com/h4llow3En/dockguard/commit/440bd22bb1ce801d99a0e18db73591048bf47acc))

### [v0.1.4](https://github.com/h4llow3En/dockguard/compare/v0.1.3...v0.1.4) (2026-03-17)

#### Fixes

* check local registry for newer image first and resolve image if only sha512
is present
([181472a](https://github.com/h4llow3En/dockguard/commit/181472a8a7f769961498fd06ba4e81cada15f6a6))

### v0.1.3 (2026-03-17)

#### Features

* ensure no updates are run when self-update is triggered
([b37c0d5](https://github.com/h4llow3En/dockguard/commit/b37c0d54ea77967a4e2dbf72172554df3c0641b0))
* implement updater
([ad81081](https://github.com/h4llow3En/dockguard/commit/ad81081c7f0c73076a91ecd94307b4fd5a6f3196))
* Add Dockerfile and build in publish workflow
([3131043](https://github.com/h4llow3En/dockguard/commit/3131043098e0f14220e3b4326bbd5b388cb0b6fc))
* Add health check for later use with docker
([e119cb1](https://github.com/h4llow3En/dockguard/commit/e119cb16566c1dc97a584b31914bd77b4f56b871))
* implement container watcher
([ebde1f3](https://github.com/h4llow3En/dockguard/commit/ebde1f3d6e99981d39400cb4c8b37460caf07007))
* logging logic and respect only ENV GUARD_SELF_UPDATE
([95676f5](https://github.com/h4llow3En/dockguard/commit/95676f5e7261765bf393a84ca43aea624c272541))

#### Fixes

* run docker image as root user since docker socket has 660 permissions on
root:docker user
([44b5b6b](https://github.com/h4llow3En/dockguard/commit/44b5b6b24d8ca0c673b953c71f8095047aa579b5))
* creating release
([d4ec7a2](https://github.com/h4llow3En/dockguard/commit/d4ec7a2f97d98525271fd32f109d9c8e8811c621))
* build and publish docker containers
([ff8cf9d](https://github.com/h4llow3En/dockguard/commit/ff8cf9d171bcf08f414cb9ec4e6f18d1d2d04a3f))
