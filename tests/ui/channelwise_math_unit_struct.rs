//! This test verifies that ChannelwiseMath cannot be derived for unit structs.

use fovea_derive::ChannelwiseMath;

#[derive(Clone, Copy, ChannelwiseMath)]
struct NoChannels;

fn main() {}
