pub mod glyph;
pub mod leptonica_ext;
pub mod tesseract_ext;

use std::fmt::Write;

use leptess::leptonica;
use leptonica_ext::PixExt;
use tesseract_ext::{BoundingBox, Tess};

pub fn recognize(img: &[u8]) -> eyre::Result<String> {
    let mut tess = Tess::new(c"./assets/tessdata", c"stam")?;

    let mut pix = leptonica::pix_read_mem(img)?;
    pix.convert_to_32()?;

    tess.set_image(&pix);
    tess.recognize()?;

    let mut w = String::new();
    for char in tess.results_iter() {
        let text = char.text();
        let BoundingBox {
            left,
            bottom,
            right,
            top,
        } = char.bounding_box();
        writeln!(&mut w, "{text} {left} {bottom} {right} {top} 0")?;
    }

    Ok(w)
}
