//! This test verifies that ChannelwiseMath cannot be derived for unions.

use fovea_derive::ChannelwiseMath;

#[derive(ChannelwiseMath)]
#[repr(C)]
union Bad {
    a: u8,
    b: u16,
}

fn main() {}
