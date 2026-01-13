use {
    crate::{FONT_SIZE, MARGIN},
    editor::FONT,
    iced::Point,
    image::{Rgba, RgbaImage},
    swash::{
        FontRef,
        scale::{Render, ScaleContext, Source, image::Image},
        zeno::Format,
    },
    teamim::{
        glyph::{self, CombiningClass, diacs},
        tesseract_ext::bounding_box::Rect,
    },
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct DiacPositionOpts {
    pub(crate) font_size: f32,
    pub(crate) margin: f32,
}

impl Default for DiacPositionOpts {
    fn default() -> Self {
        Self { font_size: FONT_SIZE, margin: MARGIN }
    }
}

pub(crate) struct Renderer {
    font: FontRef<'static>,
    ctx: ScaleContext,
    image: Image,
}

impl Renderer {
    pub(crate) fn new() -> Self {
        Self { font: *FONT, ctx: ScaleContext::new(), image: Image::new() }
    }

    #[must_use]
    pub fn position(letter: char, diac: char, rect: Rect<u32>, opts: DiacPositionOpts) -> Point {
        let DiacPositionOpts { font_size, margin } = opts;
        #[expect(clippy::cast_precision_loss)]
        let rect = rect.map(|coord| coord as f32);

        let glyph = glyph::GLYPHS.get(&diac).unwrap();

        // TODO
        let diac_height = font_size * 0.2;
        let diac_width = font_size * 0.2;

        let center = rect.left + rect.width() / 2.0;
        match glyph.combining_class {
            CombiningClass::Above => Point::new(center, rect.top - diac_height - margin),
            CombiningClass::Bottom => Point::new(center, rect.bottom + margin),
            CombiningClass::After => {
                let diac_y_precent = if letter == 'ל' {
                    0.5
                } else if diac == diacs::SOF_PASUQ.diac {
                    0.3
                } else {
                    0.1
                };
                Point::new(
                    rect.left - diac_width - margin,
                    rect.top - rect.height() * diac_y_precent,
                )
            }
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
    pub(crate) fn render(&mut self, c: char, px_size: f32, [r, g, b]: [u8; 3]) -> RgbaImage {
        self.render_into(c, px_size);
        let width = self.image.placement.width;
        let height = self.image.placement.height;
        let mut rgba = RgbaImage::new(width, height);
        let data = &self.image.data;

        for (src, dst) in data.iter().zip(rgba.pixels_mut()) {
            *dst = Rgba([r, g, b, *src]);
        }

        self.image.clear();
        rgba
    }
}
