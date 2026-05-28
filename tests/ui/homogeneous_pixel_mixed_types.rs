//! Test that HomogeneousPixel derive fails when fields have different types.

use fovea_derive::HomogeneousPixel;

#[derive(Clone, Copy, HomogeneousPixel)]
#[repr(C)]
struct MixedTypes {
    pub r: u8,
    pub g: u16,
    pub b: u8,
}

fn main() {}
