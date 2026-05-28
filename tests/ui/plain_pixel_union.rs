use fovea_derive::PlainPixel;

#[derive(PlainPixel)]
#[repr(C)]
union Bad {
    a: u8,
    b: u16,
}

fn main() {}
