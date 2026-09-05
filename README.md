# fovea-derive

[![Crates.io](https://img.shields.io/crates/v/fovea-derive.svg)](https://crates.io/crates/fovea-derive)
[![Documentation](https://docs.rs/fovea-derive/badge.svg)](https://docs.rs/fovea-derive)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/karhunen-loeve/fovea-derive/blob/main/LICENSE)

`fovea-derive` contains the procedural macros that turn pixel structs into fovea pixel types with explicit layout and semantics.

Most users should depend on `fovea`, not on this crate directly. The core crate re-exports the derives so custom pixel definitions can stay in normal fovea code.

```sh
cargo add fovea
```

Depend on `fovea-derive` directly only if you are working on fovea internals or explicitly need the macro crate.

## Derives

| Derive | What it promises |
|---|---|
| `PlainPixel` | The type has a stable byte layout suitable for byte-level access. |
| `HomogeneousPixel` | Every channel has the same channel type and can be viewed as a channel array. |
| `ZeroablePixel` | The type has an all-zero pixel value for allocation and initialization. |
| `LinearPixel` | The type supports channel-wise linear arithmetic with an explicit accumulator. |
| `WhiteChannel` | A homogeneous pixel can report its white/max channel value. |

The derives are intentionally strict. If the macro rejects a type, it is usually protecting a layout or semantic invariant that unsafe byte paths and transform bounds rely on.

## Custom pixel example

```rust,ignore
use fovea::{HomogeneousPixel, PlainPixel, ZeroablePixel};
use std::num::Saturating;

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel, ZeroablePixel)]
#[repr(C)]
pub struct BayerRg8 {
    pub value: Saturating<u8>,
}
```

Use `#[repr(C)]` for multi-field pixels and `#[repr(transparent)]` for one-field wrapper pixels. Avoid implicit layout. Pixel layout is part of the API contract.

## Linear pixels need an accumulator

Interpolation and blending often need more precision than the storage pixel. `LinearPixel` makes that accumulator explicit.

```rust,ignore
use fovea::{HomogeneousPixel, LinearPixel, PlainPixel, ZeroablePixel};
use std::num::Saturating;

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel, ZeroablePixel, LinearPixel)]
#[repr(C)]
#[linear(accumulator = RgbF32)]
pub struct Rgb8 {
    pub r: Saturating<u8>,
    pub g: Saturating<u8>,
    pub b: Saturating<u8>,
}

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel, ZeroablePixel, LinearPixel)]
#[repr(C)]
#[linear(accumulator = Self)]
pub struct RgbF32 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}
```

Do not derive `LinearPixel` just because arithmetic is possible. Derive it only when blending or interpolation is meaningful for the pixel semantics. Gamma-encoded sRGB pixel types intentionally do not implement `LinearSpace`.

### When arithmetic is meaningful but interpolation is not

Some types sit between the two: a weighted sum of their values is real work, but mixing two *neighbouring* values is not. A raw Bayer sensor sample is the standard case — averaging a neighbourhood estimates a defect pixel or a noise floor, while interpolating between adjacent samples blends different colour channels. Add the bare `no_space` flag and the macro emits everything except the `LinearSpace` marker, so `blend()` and `Bilinear` resize become compile errors while convolution and filtering keep working.

```rust,ignore
#[derive(Clone, Copy, PlainPixel, HomogeneousPixel, ZeroablePixel, LinearPixel)]
#[repr(transparent)]
#[linear(accumulator = MonoF32, no_space)]
pub struct BayerRggb8(Saturating<u8>);
```

## What this crate is not

`fovea-derive` does not define the pixel model. The model lives in `fovea::pixel`; this crate only automates correct implementations. Read the `fovea` crate docs first unless you are debugging macro behavior.

## Crate ecosystem

| Crate | Purpose |
|---|---|
| `fovea` | Core crate and normal home of the derive re-exports. |
| `fovea-derive` | Procedural macro implementation crate. |
| `fovea-examples` | Repo-only examples, including custom-pixel patterns. |

## License

Licensed under the [MIT License](https://github.com/karhunen-loeve/fovea-derive/blob/main/LICENSE).
