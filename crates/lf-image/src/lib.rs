//! Image processing for Little Falls, with no dependencies so it is fast
//! to build and fully unit tested:
//!
//! - `mips`: full mipmap chains for 8-bit textures (scanned assets ship
//!   without them, and unmipmapped textures shimmer in motion)
//! - `half`: f32 to f16 conversion for GPU float textures
//! - `ibl`: image based lighting from an equirectangular HDRI: a skybox
//!   cubemap, a diffuse irradiance cubemap and a GGX prefiltered specular
//!   cubemap

// Pixel math indexes several parallel channel arrays.
#![allow(clippy::needless_range_loop)]

pub mod half;
pub mod ibl;
pub mod mips;
