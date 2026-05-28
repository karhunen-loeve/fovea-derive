//! This test verifies that LinearPixel cannot be derived for structs with no fields.

use fovea_derive::LinearPixel;

#[derive(LinearPixel)]
#[linear(accumulator = Self)]
struct Empty {}

fn main() {}
