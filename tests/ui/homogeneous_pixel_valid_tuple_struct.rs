use fovea_derive::{PlainPixel, HomogeneousPixel};
use std::num::Saturating;

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel)]
#[repr(C)]
pub struct ValidTupleRgb(Saturating<u8>, Saturating<u8>, Saturating<u8>);

#[derive(Clone, Copy, PlainPixel, HomogeneousPixel)]
#[repr(C)]
pub struct ValidTupleMono(u16);

fn main() {}
