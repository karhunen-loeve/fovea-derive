//! `no_space` is a bare flag; giving it a value must be rejected rather than
//! silently ignored — the difference between emitting `LinearSpace` and not
//! is the difference between allowing and forbidding interpolation.

use fovea_derive::LinearPixel;

#[derive(LinearPixel)]
#[linear(accumulator = Acc, no_space = Acc)]
pub struct Bad {
    pub x: u8,
}

pub struct Acc {
    pub x: f32,
}

fn main() {}
