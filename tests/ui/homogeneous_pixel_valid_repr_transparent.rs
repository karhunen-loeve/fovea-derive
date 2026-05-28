use irys_cv_derive::{PlainPixel, HomogeneousPixel};
use std::num::Saturating;

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel)]
#[repr(transparent)]
pub struct ValidTransparentPixel {
    pub value: Saturating<u8>,
}

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel)]
#[repr(transparent)]
pub struct ValidTransparentTuple(u16);

fn main() {}
