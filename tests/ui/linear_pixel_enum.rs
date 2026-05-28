//! This test verifies that LinearPixel cannot be derived for enums.

use irys_cv_derive::LinearPixel;

#[derive(Clone, Copy, LinearPixel)]
#[linear(accumulator = Self)]
pub enum BadPixel {
    Red,
    Green,
    Blue,
}

fn main() {}
