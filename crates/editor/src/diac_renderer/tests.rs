use image::{ImageFormat, Rgba, RgbaImage};

use crate::FONT_SIZE;

const PASHTA: char = '\u{0599}';

#[test]
fn draw_glyph() -> eyre::Result<()> {
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let size = FONT_SIZE as u32 * 2;
    let mut img = RgbaImage::from_pixel(size, size, Rgba([0xFF, 0xFF, 0xFF, 0xFF]));
    let mut renderer = super::Renderer::new();
    renderer.draw_glyph(&mut img, PASHTA, [size / 2, size / 2], FONT_SIZE);
    img.save_with_format("../../../temp/saved-tests/draw_glyph_test.png", ImageFormat::Png)?;
    Ok(())
}
