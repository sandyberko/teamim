use image::{Rgba, RgbaImage};
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
