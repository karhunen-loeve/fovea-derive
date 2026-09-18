# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] — 2026-09-18

Released for ecosystem-version alignment; no functional changes.

## [0.4.0] — 2026-09-05

### Added

- `#[linear(accumulator = T, no_space)]` — a bare `no_space` flag on the
  `#[derive(LinearPixel)]` helper attribute. With it the macro emits
  `Add` / `Sub` / `Mul`, `LinearPixel`, and `FromLinear` as before but
  **omits `LinearSpace`**, so weighted sums (convolution, filters,
  template matching) stay available while interpolation and blending
  (`blend()`, `Bilinear` resize, the `LinearCombine` / `Blend` combiners)
  become compile errors on the type.
  This is the split raw Bayer CFA samples need — arithmetic on them is
  meaningful, interpolation *between* them mixes colour channels — and
  the same shape fits Lab / Luv and YCbCr later. Purely additive: without
  the flag the macro behaves exactly as it did, `LinearSpace` included.

## [0.3.0] — 2026-07-27

Released in lockstep with the rest of the `fovea` ecosystem. The
proc-macro implementations and the generated code are **unchanged** from
`0.2.0`; the crates share a workspace version so that a `fovea 0.3.0`
resolves against a matching `fovea-derive`.

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

[0.5.0]: https://github.com/karhunen-loeve/fovea-derive/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/karhunen-loeve/fovea-derive/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/karhunen-loeve/fovea-derive/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/karhunen-loeve/fovea-derive/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/karhunen-loeve/fovea-derive/releases/tag/v0.1.1
