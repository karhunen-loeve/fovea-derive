//! This test verifies that ZeroablePixel cannot be derived for unions.

use irys_cv_derive::ZeroablePixel;

#[derive(Copy, Clone, ZeroablePixel)]
pub union BadPixel {
    pub a: u8,
    pub b: u16,
}

fn main() {}
