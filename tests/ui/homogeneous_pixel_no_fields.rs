//! Test that HomogeneousPixel derive fails when the struct has no fields.

use fovea_derive::HomogeneousPixel;

#[derive(Clone, Copy, HomogeneousPixel)]
#[repr(C)]
struct NoFields {}

fn main() {}
