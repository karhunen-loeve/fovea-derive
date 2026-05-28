//! PLAN §3.5: verify that an unknown per-field key inside
//! `#[linear(..)]` produces a diagnostic-quality error.

use fovea_derive::LinearPixel;

#[derive(LinearPixel)]
#[linear(accumulator = Acc)]
pub struct Bad {
    #[linear(frobnicate)]
    pub x: u8,
}

pub struct Acc {
    pub x: f32,
}

fn main() {}
