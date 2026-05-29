# fovea-derive

[![Crates.io](https://img.shields.io/crates/v/fovea-derive.svg)](https://crates.io/crates/fovea-derive)
[![Documentation](https://docs.rs/fovea-derive/badge.svg)](https://docs.rs/fovea-derive)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/karhunen-loeve/fovea-derive/blob/main/LICENSE)

`fovea-derive` provides the procedural macros used by [`fovea`](https://github.com/karhunen-loeve/fovea) to derive pixel layout and arithmetic traits.

Most users should depend on `fovea` and use the derive macros re-exported from that crate. Depend on `fovea-derive` directly only if you are working on the fovea internals or need the macro crate explicitly.

```toml
[dependencies]
fovea = "0.1.1"
```

## Derives

| Derive | Purpose |
|---|---|
| `PlainPixel` | Declares that a pixel has a stable byte layout suitable for byte-level access. |
| `HomogeneousPixel` | Declares that all channels have the same channel type. |
| `ZeroablePixel` | Generates a zero value for image allocation and initialization. |
| `LinearPixel` | Generates linear-space arithmetic support with an explicit accumulator type. |

## Example

```rust,ignore
use fovea::{HomogeneousPixel, PlainPixel, ZeroablePixel};
use std::num::Saturating;

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel, ZeroablePixel)]
#[repr(C)]
pub struct Rgb8 {
    pub r: Saturating<u8>,
    pub g: Saturating<u8>,
    pub b: Saturating<u8>,
}
```

The derives intentionally enforce fovea's pixel model: memory layout is explicit, channel roles are named, and invalid trait implementations should fail during compilation rather than at runtime.

## Part of the fovea project

- Core crate: [`fovea`](https://github.com/karhunen-loeve/fovea)
- End-to-end demos: [`fovea-examples`](https://github.com/karhunen-loeve/fovea-examples)

## License

Licensed under the [MIT License](https://github.com/karhunen-loeve/fovea-derive/blob/main/LICENSE).
