use irys_cv_derive::PlainPixel;

#[derive(Clone, Copy, PlainPixel)]
#[repr(transparent)]
pub struct ValidTransparentPixel {
    pub value: u16,
}

fn main() {}
