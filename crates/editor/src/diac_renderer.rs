#[cfg(test)]
mod tests;

use image::{Pixel as _, Rgba, RgbaImage};
use swash::{
    FontRef,
    scale::{Render, ScaleContext, Source, image::Image},
    zeno::Format,
};

const GUTTMAN: &[u8] = include_bytes!("../../../assets/fonts/Guttman_Stam.ttf");

pub(crate) struct Renderer {
    font: FontRef<'static>,
    ctx: ScaleContext,
    image: Image,
}

impl Renderer {
    pub(crate) fn new() -> Self {
        Self {
            font: FontRef::from_index(GUTTMAN, 0).expect("invalid font data"),
            ctx: ScaleContext::new(),
            image: Image::new(),
        }
    }

    pub(crate) fn draw_glyph(
        &mut self,
        bottom: &mut RgbaImage,
        c: char,
        offset: [u32; 2],
        px_size: f32,
    ) {
        self.render_into(c, px_size);

        let Image { placement, data, .. } = &self.image;

        let bottom_dims = bottom.dimensions();
        let top_dims = (placement.width, placement.height);
        let [x, y] = offset.map(i64::from);
        let [x, y] = [x + i64::from(placement.left), y - i64::from(placement.top)];

        // Crop our top image if we're going out of bounds
        let (
            origin_bottom_x,
            origin_bottom_y,
            origin_top_x,
            origin_top_y,
            range_width,
            range_height,
        ) = overlay_bounds_ext(bottom_dims, top_dims, x, y);

        for y in 0..range_height {
            for x in 0..range_width {
                let top_alpha =
                    data[(((origin_top_y + y) * top_dims.0) + (origin_top_x + x)) as usize];
                let top_pixel = Rgba([0xFF, 0, 0, top_alpha]);

                let mut bottom_pixel = *bottom.get_pixel(origin_bottom_x + x, origin_bottom_y + y);
                bottom_pixel.blend(&top_pixel);

                bottom.put_pixel(origin_bottom_x + x, origin_bottom_y + y, bottom_pixel);
            }
        }
        self.image.clear();
    }

    fn render_into(&mut self, c: char, px_size: f32) {
        let glyph_id = self.font.charmap().map(c);
        // Rasterize glyph
        let mut scaler = self.ctx.builder(self.font).size(px_size).hint(true).build();
        if !Render::new(&[Source::Outline]).format(Format::Alpha).render_into(
            &mut scaler,
            glyph_id,
            &mut self.image,
        ) {
            panic!("outline should exist for glyph {glyph_id}");
        }
    }
    pub(crate) fn render(&mut self, c: char, px_size: f32) -> RgbaImage {
        self.render_into(c, px_size);
        let width = self.image.placement.width;
        let height = self.image.placement.height;
        let mut rgba = RgbaImage::new(width, height);
        let data = &self.image.data;

        for (src, dst) in data.iter().zip(rgba.pixels_mut()) {
            *dst = Rgba([0xFF, 0, 0, *src]); // black text, alpha from coverage
        }

        self.image.clear();
        rgba
    }
}

/// Calculate the region that can be copied from top to bottom.
///
/// Given image size of bottom and top image, and a point at which we want to place the top image
/// onto the bottom image, how large can we be? Have to wary of the following issues:
/// * Top might be larger than bottom
/// * Overflows in the computation
/// * Coordinates could be completely out of bounds
///
/// The returned value is of the form:
///
/// `(origin_bottom_x, origin_bottom_y, origin_top_x, origin_top_y, x_range, y_range)`
///
/// The main idea is to do computations on i64's and then clamp to image dimensions.
/// In particular, we want to ensure that all these coordinate accesses are safe:
/// 1. `bottom.get_pixel(origin_bottom_x + [0..x_range), origin_bottom_y + [0..y_range))`
/// 2. `top.get_pixel(origin_top_y + [0..x_range), origin_top_y + [0..y_range))`
fn overlay_bounds_ext(
    (bottom_width, bottom_height): (u32, u32),
    (top_width, top_height): (u32, u32),
    x: i64,
    y: i64,
) -> (u32, u32, u32, u32, u32, u32) {
    // Return a predictable value if the two images don't overlap at all.
    if x > i64::from(bottom_width)
        || y > i64::from(bottom_height)
        || x.saturating_add(i64::from(top_width)) <= 0
        || y.saturating_add(i64::from(top_height)) <= 0
    {
        return (0, 0, 0, 0, 0, 0);
    }

    // Find the maximum x and y coordinates in terms of the bottom image.
    let max_x = x.saturating_add(i64::from(top_width));
    let max_y = y.saturating_add(i64::from(top_height));

    // Clip the origin and maximum coordinates to the bounds of the bottom image.
    // Casting to a u32 is safe because both 0 and `bottom_{width,height}` fit
    // into 32-bits.
    let max_inbounds_x = max_x.clamp(0, i64::from(bottom_width)) as u32;
    let max_inbounds_y = max_y.clamp(0, i64::from(bottom_height)) as u32;
    let origin_bottom_x = x.clamp(0, i64::from(bottom_width)) as u32;
    let origin_bottom_y = y.clamp(0, i64::from(bottom_height)) as u32;

    // The range is the difference between the maximum inbounds coordinates and
    // the clipped origin. Unchecked subtraction is safe here because both are
    // always positive and `max_inbounds_{x,y}` >= `origin_{x,y}` due to
    // `top_{width,height}` being >= 0.
    let x_range = max_inbounds_x - origin_bottom_x;
    let y_range = max_inbounds_y - origin_bottom_y;

    // If x (or y) is negative, then the origin of the top image is shifted by -x (or -y).
    let origin_top_x = x.saturating_mul(-1).clamp(0, i64::from(top_width)) as u32;
    let origin_top_y = y.saturating_mul(-1).clamp(0, i64::from(top_height)) as u32;

    (origin_bottom_x, origin_bottom_y, origin_top_x, origin_top_y, x_range, y_range)
}
