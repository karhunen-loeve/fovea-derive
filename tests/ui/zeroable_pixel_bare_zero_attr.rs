//! This test verifies that bare `#[zero]` (without parentheses) is rejected.

use irys_cv_derive::ZeroablePixel;

#[derive(Clone, Copy, ZeroablePixel)]
pub struct BadPixel {
    #[zero]
    pub x: u8,
}

fn main() {}
