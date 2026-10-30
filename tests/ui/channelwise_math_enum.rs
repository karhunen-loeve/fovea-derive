//! This test verifies that ChannelwiseMath cannot be derived for enums.

use fovea_derive::ChannelwiseMath;

#[derive(Clone, Copy, ChannelwiseMath)]
#[repr(C)]
pub enum BadPixel {
    Red,
    Green,
    Blue,
}

fn main() {}
