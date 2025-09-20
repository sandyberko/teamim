use std::ops::{Deref, DerefMut};

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use image::{ImageBuffer, Rgba};

use crate::FONT_SIZE;

pub fn draw_text<Container>(
    img: &mut ImageBuffer<Rgba<u8>, Container>,
    position: [i32; 2],
    text: &str,
    font_size: f32,
) where
    Container: Deref<Target = [u8]> + DerefMut,
{
    // A FontSystem provides access to detected system fonts, create one per application
    let mut font_system = FontSystem::new();

    // A SwashCache stores rasterized glyphs, create one per application
    let mut swash_cache = SwashCache::new();

    // Text metrics indicate the font size and line height of a buffer
    let metrics = Metrics::new(font_size, font_size);

    // A Buffer provides shaping and layout for a UTF-8 string, create one per text widget
    let mut buffer = Buffer::new(&mut font_system, metrics);

    // Borrow buffer together with the font system for more convenient method calls
    let mut buffer = buffer.borrow_with(&mut font_system);

    // Set a size for the text buffer, in pixels
    // buffer.set_size(Some(80.0), Some(25.0));

    // Attributes indicate what font to choose
    let attrs = Attrs::new();

    // Add some text!
    buffer.set_text(text, &attrs, Shaping::Advanced);

    // Perform shaping as desired
    buffer.shape_until_scroll(true);

    // Inspect the output runs
    // for run in buffer.layout_runs() {
    //     for glyph in run.glyphs.iter() {
    //         println!("{:#?}", glyph);
    //     }
    // }

    // Create a default text color
    let text_color = Color::rgb(0, 0, 0);

    // Draw the buffer (for performance, instead use SwashCache directly)
    buffer.draw(&mut swash_cache, text_color, |x, y, w, h, color| {
        let x = position[0] + x;
        let y = position[1] + y;

        let image_width = img.width() as i32;
        let image_height = img.height() as i32;

        for dy in 0..h {
            for dx in 0..w {
                let px = x + dx as i32;
                let py = y + dy as i32;

                // Skip if out of bounds
                if px < 0 || py < 0 || px >= image_width || py >= image_height {
                    continue;
                }

                let pixel = img.get_pixel_mut(px as u32, py as u32);

                // Convert cosmic_text::Color (floats) into u8 RGBA
                let src = Rgba(color.as_rgba());

                // Alpha blend: src over dst
                let alpha = src[3] as f32 / 255.0;
                for i in 0..3 {
                    pixel[i] = ((1.0 - alpha) * (pixel[i] as f32) + alpha * (src[i] as f32)) as u8;
                }
                // Update alpha too (optional, depending on use case)
                pixel[3] = 255;
            }
        }
    });
}
