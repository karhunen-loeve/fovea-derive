//! Test that HomogeneousPixel derive fails when #[repr(C)] is missing.

use fovea_derive::HomogeneousPixel;

#[derive(Clone, Copy, HomogeneousPixel)]
struct MissingReprC {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

fn main() {}
