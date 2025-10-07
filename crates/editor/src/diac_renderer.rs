use std::ops::{Deref, DerefMut};

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use image::{ImageBuffer, Rgba};

pub(crate) struct Renderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl Renderer {
    pub(crate) fn new() -> Self {
        Self { font_system: FontSystem::new(), swash_cache: SwashCache::new() }
    }

    pub(crate) fn draw_text<Container>(
        &mut self,
        img: &mut ImageBuffer<Rgba<u8>, Container>,
        position: [i32; 2],
        text: &str,
        font_size: f32,
    ) where
        Container: Deref<Target = [u8]> + DerefMut,
    {
        // Text metrics indicate the font size and line height of a buffer
        let metrics = Metrics::new(font_size, font_size);

        // A Buffer provides shaping and layout for a UTF-8 string, create one per text widget
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        // Borrow buffer together with the font system for more convenient method calls
        let mut buffer = buffer.borrow_with(&mut self.font_system);

        // Set a size for the text buffer, in pixels
        // buffer.set_size(Some(80.0), Some(25.0));

        // Attributes indicate what font to choose
        let attrs = Attrs::new();

        // Add some text!
        buffer.set_text(text, &attrs, Shaping::Advanced);

        // Perform shaping as desired
        buffer.shape_until_scroll(true);

        // Inspect the output run
        // for run in buffer.layout_runs() {
        //     for glyph in run.glyphs.iter() {
        //         println!("{:#?}", glyph);
        //     }
        // }

        // Create a default text color
        let text_color = Color::rgb(0, 0, 0);

        // Draw the buffer (for performance, instead use SwashCache directly)
        buffer.draw(&mut self.swash_cache, text_color, |x, y, w, h, color| {
            let x = position[0] + x;
            let y = position[1] + y;

            let image_width = img.width();
            let image_height = img.height();

            for dy in 0..h {
                for dx in 0..w {
                    let Some(px) = dx.checked_add_signed(x) else { continue };
                    let Some(py) = dy.checked_add_signed(y) else { continue };

                    // Skip if out of bounds
                    if px >= image_width || py >= image_height {
                        continue;
                    }

                    let pixel = img.get_pixel_mut(px, py);

                    // Convert cosmic_text::Color (floats) into u8 RGBA
                    let src = Rgba(color.as_rgba());

                    // Alpha blend: src over dst
                    let alpha = f32::from(src[3]) / 255.0;
                    for i in 0..3 {
                        let val = (1.0 - alpha) * f32::from(pixel[i]) + alpha * f32::from(src[i]);
                        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        {
                            pixel[i] = val as u8;
                        }
                    }
                    // Update alpha too (optional, depending on use case)
                    pixel[3] = 255;
                }
            }
        });
    }
}
