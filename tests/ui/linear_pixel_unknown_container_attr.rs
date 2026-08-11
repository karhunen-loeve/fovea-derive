//! An unknown container-level key inside `#[linear(..)]` must name the keys
//! that are accepted.

use fovea_derive::LinearPixel;

#[derive(LinearPixel)]
#[linear(accumulator = Acc, frobnicate)]
pub struct Bad {
    pub x: u8,
}

pub struct Acc {
    pub x: f32,
}

fn main() {}
