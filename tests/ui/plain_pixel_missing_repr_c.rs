//! Test that PlainPixel derive fails when #[repr(C)] is missing.

use fovea_derive::PlainPixel;

#[derive(Clone, Copy, PlainPixel)]
struct MissingReprC {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

fn main() {}
