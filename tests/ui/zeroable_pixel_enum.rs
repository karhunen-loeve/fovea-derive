//! This test verifies that ZeroablePixel cannot be derived for enums.

use irys_cv_derive::ZeroablePixel;

#[derive(Clone, Copy, ZeroablePixel)]
pub enum BadPixel {
    Red,
    Green,
    Blue,
}

fn main() {}
