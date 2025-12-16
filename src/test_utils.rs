mod positions;
pub use positions::POSITIONS;

use {
    crate::TeamimCtx,
    image::{ImageFormat, ImageReader, RgbaImage},
    std::{ffi::CStr, io::Cursor, sync::LazyLock},
};

pub const DIAC_COLOR: [u8; 3] = [0xFF, 0, 0];

pub fn ctx() -> eyre::Result<TeamimCtx> {
    TeamimCtx::new(
        CStr::from_bytes_with_nul(
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tessdata/", '\0').as_bytes(),
        )
        .unwrap(),
    )
}

#[macro_export]
macro_rules! img_path {
    () => {
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/images/N5/007.jpg")
    };
}
pub use img_path;

pub static IMAGE: LazyLock<RgbaImage> = LazyLock::new(|| {
    let bytes = include_bytes!(img_path!());
    ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg)
        .decode()
        .expect("invalid image")
        .to_rgba8()
});
