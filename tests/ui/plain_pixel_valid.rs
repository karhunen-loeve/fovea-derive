use irys_cv_derive::PlainPixel;

#[derive(Clone, Copy, PlainPixel)]
#[repr(C)]
pub struct ValidPixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

fn main() {}
