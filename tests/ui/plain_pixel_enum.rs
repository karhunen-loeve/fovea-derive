//! This test verifies that PlainPixel cannot be derived for enums.

use fovea_derive::PlainPixel;

#[derive(Clone, Copy, PlainPixel)]
#[repr(C)]
pub enum BadPixel {
    Red,
    Green,
    Blue,
}

fn main() {}
