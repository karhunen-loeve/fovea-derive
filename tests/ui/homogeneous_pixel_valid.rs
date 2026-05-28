use irys_cv_derive::{PlainPixel, HomogeneousPixel};
use std::num::Saturating;

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel)]
#[repr(C)]
pub struct ValidRgb {
    pub r: Saturating<u8>,
    pub g: Saturating<u8>,
    pub b: Saturating<u8>,
}

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel)]
#[repr(C)]
pub struct ValidRgba {
    pub r: Saturating<u8>,
    pub g: Saturating<u8>,
    pub b: Saturating<u8>,
    pub a: Saturating<u8>,
}

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel)]
#[repr(C)]
pub struct ValidMono {
    pub value: u16,
}

fn main() {}
