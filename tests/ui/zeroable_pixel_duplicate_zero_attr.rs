//! This test verifies that duplicate `#[zero(...)]` attributes on a single field are rejected.

use irys_cv_derive::ZeroablePixel;

#[derive(Clone, Copy, ZeroablePixel)]
pub struct BadPixel {
    #[zero(default)]
    #[zero(42)]
    pub x: u8,
}

fn main() {}
