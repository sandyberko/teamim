#![expect(dead_code, unused_variables)]

use imageproc::{filter::gaussian_blur_f32, image::GrayImage, noise::gaussian_noise_mut};
use rand::Rng;
use std::f32::consts::PI;

const COMPLEXITY: u32 = 7u32;

pub(crate) fn augment(img: &mut GrayImage, rng: &mut impl Rng, bulge: Bulge) {
    // dilate_mut(img, Norm::L1, 1);

    *img = gaussian_blur_f32(img, rng.random_range(0.1..2.0));

    gaussian_noise_mut(
        img,
        (COMPLEXITY - 1).into(),
        (10 * COMPLEXITY - 10).into(),
        (5 * COMPLEXITY - 5).into(),
    );

    // *img = warp_with(img, move |x, y| bulge.warp(x, y), Interpolation::Bilinear, Luma([0u8]));
}

#[derive(Clone, Copy)]
pub struct Bulge {
    cx: f32,
    cy: f32,
    strength: f32,
    radius: f32,
}

#[expect(clippy::cast_precision_loss)]
impl Bulge {
    pub fn new(rng: &mut impl Rng, w: u32, h: u32) -> Self {
        // Random center within the image
        let cx = rng.random_range(0.3..0.7) * w as f32;
        let cy = rng.random_range(0.3..0.7) * h as f32;

        let strength = rng.random_range(-0.1..0.1); // Negative for concave, positive for convex
        let radius = rng.random_range(0.6..1.0) * w.min(h) as f32;
        Self { cx, cy, strength, radius }
    }

    pub fn warp(&self, x: f32, y: f32) -> (f32, f32) {
        let Self { cx, cy, strength, radius } = *self;

        let dx = x - cx;
        let dy = y - cy;
        let r = (dx * dx + dy * dy).sqrt();
        if r < radius {
            let theta = (r / radius) * PI;
            let displacement = strength * (theta.sin());
            let scale = 1.0 + displacement;
            (cx + dx * scale, cy + dy * scale)
        } else {
            (x, y)
        }
    }
}
