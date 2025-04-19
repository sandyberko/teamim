use std::path::Path;

#[cfg(windows)]
const TESS_DIR: &str = "C:\\Program Files\\Tesseract-OCR";
#[cfg(not(windows))]
const TESS_DIR: &str = "/usr/bin";

#[must_use]
pub fn tess_dir() -> &'static Path {
    Path::new(TESS_DIR)
}

#[cfg(windows)]
const TESSDATA_DIR: &str = "C:\\Program Files\\Tesseract-OCR\\tessdata";
#[cfg(not(windows))]
const TESSDATA_DIR: &str = "/usr/share/tessdata";

#[must_use]
pub fn tessdata_dir() -> &'static Path {
    Path::new(TESSDATA_DIR)
}
