use iced::Point;
use image::{Rgba, RgbaImage};
use swash::{
    FontRef,
    scale::{Render, ScaleContext, Source, image::Image},
    zeno::Format,
};
use teamim::{
    diac,
    glyph::{self, CombiningClass},
    tesseract_ext::bounding_box::Rect,
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

    #[must_use]
    pub fn position(&self, letter: char, diac: char, rect: Rect<u32>, font_size: f32) -> Point {
        #[expect(clippy::cast_precision_loss)]
        let rect = rect.map(|coord| coord as f32);

        let glyph = glyph::GLYPHS.get(&diac).unwrap();

        // TODO
        let diac_height = font_size * 0.2;
        let diac_width = font_size * 0.2;
        let margin = font_size * 0.05;

        let center = rect.left + rect.width() / 2.0;
        match (letter, glyph.combining_class) {
            (_, CombiningClass::Above) => Point::new(center, rect.top - diac_height - margin),
            (_, CombiningClass::Bottom) => Point::new(center, rect.bottom + margin),
            (_, CombiningClass::After) => Point::new(rect.left - diac_width - margin, rect.top),
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
