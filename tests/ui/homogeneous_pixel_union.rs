use fovea_derive::HomogeneousPixel;

#[derive(HomogeneousPixel)]
#[repr(C)]
union Bad {
    a: u8,
    b: u16,
}

fn main() {}
