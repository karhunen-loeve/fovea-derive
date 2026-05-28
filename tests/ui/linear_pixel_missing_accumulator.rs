//! This test verifies that LinearPixel requires #[linear(accumulator = Type)] attribute.

use irys_cv_derive::LinearPixel;

#[derive(LinearPixel)]
pub struct Rgb8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

fn main() {}
