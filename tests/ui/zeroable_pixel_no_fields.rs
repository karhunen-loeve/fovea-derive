//! This test verifies that ZeroablePixel cannot be derived for structs with no fields.

use fovea_derive::ZeroablePixel;

#[derive(Clone, Copy, ZeroablePixel)]
pub struct Empty {}

fn main() {}
