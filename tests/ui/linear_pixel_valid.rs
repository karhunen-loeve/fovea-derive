//! Reference example of valid LinearPixel derive usage.
//! This file is not run through trybuild (fovea is not a dev-dependency),
//! but serves as documentation for correct usage.

use fovea_derive::LinearPixel;

/// Storage pixel with an external accumulator type.
#[derive(Clone, Copy, LinearPixel)]
#[repr(C)]
#[linear(accumulator = RgbF32)]
pub struct Rgb8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Float pixel that is its own accumulator.
#[derive(Clone, Copy, LinearPixel)]
#[repr(C)]
#[linear(accumulator = Self)]
pub struct RgbF32 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// Tuple struct variant.
#[derive(Clone, Copy, LinearPixel)]
#[repr(C)]
#[linear(accumulator = Self)]
pub struct MonoF32(pub f32);

fn main() {}

