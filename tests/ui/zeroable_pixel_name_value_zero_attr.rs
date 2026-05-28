//! This test verifies that `#[zero = ...]` name-value form is rejected
//! with guidance to use `#[zero(...)]` instead.

use irys_cv_derive::ZeroablePixel;

#[derive(Clone, Copy, ZeroablePixel)]
pub struct BadPixel {
    #[zero = "default"]
    pub x: u8,
}

fn main() {}
