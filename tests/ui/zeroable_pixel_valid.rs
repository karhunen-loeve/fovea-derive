use fovea_derive::ZeroablePixel;
use std::num::Saturating;

#[derive(Clone, Copy, ZeroablePixel)]
#[repr(C)]
pub struct ValidRgb {
    pub r: Saturating<u8>,
    pub g: Saturating<u8>,
    pub b: Saturating<u8>,
}

#[derive(Clone, Copy, ZeroablePixel)]
#[repr(C)]
pub struct ValidRgba {
    pub r: Saturating<u8>,
    pub g: Saturating<u8>,
    pub b: Saturating<u8>,
    pub a: Saturating<u8>,
}

#[derive(Clone, Copy, ZeroablePixel)]
#[repr(C)]
pub struct ValidMono {
    pub value: u16,
}

#[derive(Clone, Copy, ZeroablePixel)]
#[repr(transparent)]
pub struct ValidWrapper(u8);

#[derive(Clone, Copy, ZeroablePixel)]
#[repr(C)]
pub struct ValidFloat {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

fn main() {}
