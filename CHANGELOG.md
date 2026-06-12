# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] — 2026-06-12

### Changed

- Rewrote the README with a clearer hook and richer derive documentation.

Released in lockstep with the rest of the `fovea` ecosystem; no functional
changes to the proc-macro implementations.

## [0.1.1] — 2026-05-29

First real public release. `0.1.0` was a name-reservation placeholder.

### Added

- Initial public release of the `fovea-derive` proc-macro crate.
- `#[derive(PlainPixel)]` — declares stable byte layout for a pixel type.
- `#[derive(HomogeneousPixel)]` — declares that all channels share the
  same channel type.
- `#[derive(ZeroablePixel)]` — generates a `ZERO` constant for image
  allocation and initialisation.
- `#[derive(LinearPixel)]` — generates linear-space arithmetic
  support with an explicit accumulator type.
- `trybuild` UI tests covering valid and invalid derive inputs.

[0.2.0]: https://github.com/karhunen-loeve/fovea-derive/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/karhunen-loeve/fovea-derive/releases/tag/v0.1.1
