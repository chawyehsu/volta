# Changelog

## [2.1.0](https://github.com/chawyehsu/volta/compare/v2.0.3...v2.1.0) (2026-11-16)


### Features

* add self-update command ([#108](https://github.com/chawyehsu/volta/issues/108)) ([c4e7d4b](https://github.com/chawyehsu/volta/commit/c4e7d4b45267e7525502ff0b5c9cceae8f787d12))
* **self-update:** support updating to a specified version ([#124](https://github.com/chawyehsu/volta/issues/124)) ([2ff509f](https://github.com/chawyehsu/volta/commit/2ff509f5c5a88fea0c1cd3f5df736d661de6a800))
* **shims:** create tool shims on demand during install and pin ([#125](https://github.com/chawyehsu/volta/issues/125)) ([3d40b1b](https://github.com/chawyehsu/volta/commit/3d40b1b7901221f7d7e5193099b55b1620a7396c))
* support uninstalling tools ([#114](https://github.com/chawyehsu/volta/issues/114)) ([05fe410](https://github.com/chawyehsu/volta/commit/05fe4101c21fab1eb2b6d3a678bf87d1856e5002))
* **uninstall:** remove tool shims once no remaining reference needs them ([#126](https://github.com/chawyehsu/volta/issues/126)) ([a1cc5ea](https://github.com/chawyehsu/volta/commit/a1cc5ea60356f8c0f45e97db370a2647792bb965))


### Bug Fixes

* **list:** exclude binless packages from tool list ([#127](https://github.com/chawyehsu/volta/issues/127)) ([c96cbb9](https://github.com/chawyehsu/volta/commit/c96cbb99965b647564a770260421eacf06f32bb3))
* **list:** treat pnpm as package without feature flag ([#128](https://github.com/chawyehsu/volta/issues/128)) ([ae61901](https://github.com/chawyehsu/volta/commit/ae61901fdd5378c7d204587e8200ded7262ecada))
* **pnpm:** fall back to .mjs launcher entrypoint when .cjs is absent ([#116](https://github.com/chawyehsu/volta/issues/116)) ([4c2f4e8](https://github.com/chawyehsu/volta/commit/4c2f4e843ec914d34e6622fa32e419736ace1743))
* **setup:** write Windows user Path via registry instead of setx ([#130](https://github.com/chawyehsu/volta/issues/130)) ([d5eaf62](https://github.com/chawyehsu/volta/commit/d5eaf62eb764f09b1e749039fc0ec26b3b262b2a))

## [2.0.3](https://github.com/chawyehsu/volta/compare/v2.0.2...v2.0.3) (2026-06-03)


### Bug Fixes

* **core:** eliminate Windows fork bomb caused by recursive shim execution ([#41](https://github.com/chawyehsu/volta/issues/41)) ([7d3789c](https://github.com/chawyehsu/volta/commit/7d3789cdc53491072fa5e81bd7ee22b8083791b2))
* **error:** correct error message for `NpmVersionNotFound` ([cd015e5](https://github.com/chawyehsu/volta/commit/cd015e5bb88d1b32de9579dd5eb3ec428461b99a))
* **hook:** ensure relative path bin hook parsed correctly on Windows ([99c9488](https://github.com/chawyehsu/volta/commit/99c94888684fc4a33d55862f6a4e3c3cd93a5d38))
