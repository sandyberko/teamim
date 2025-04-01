use std::path::Path;

const TESS_DIR: &str = "C:\\Program Files\\Tesseract-OCR";
#[must_use]
pub fn tess_dir() -> &'static Path {
    Path::new(TESS_DIR)
}
