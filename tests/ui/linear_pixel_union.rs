use fovea_derive::LinearPixel;

#[derive(LinearPixel)]
#[linear(accumulator = Self)]
union Bad {
    a: u8,
    b: u16,
}

fn main() {}
