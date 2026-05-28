//! This test verifies that LinearPixel cannot be derived for structs with no fields.

use irys_cv_derive::LinearPixel;

#[derive(LinearPixel)]
#[linear(accumulator = Self)]
struct Empty {}

fn main() {}
