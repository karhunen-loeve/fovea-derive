//! This test verifies that HomogeneousPixel cannot be derived for enums.

use irys_cv_derive::HomogeneousPixel;

#[derive(Clone, Copy, HomogeneousPixel)]
#[repr(C)]
pub enum BadPixel {
    Red,
    Green,
    Blue,
}

fn main() {}
