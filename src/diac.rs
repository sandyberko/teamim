use {crate::tesseract_ext::bounding_box::Rect, std::borrow::Cow};

pub type Result = core::result::Result<Rect<u32>, DiacMiss>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiacMiss {
    pub top: u32,
    pub missing_text: Cow<'static, str>,
}

pub const fn miss(top: u32, missing_text: &'static str) -> Result {
    Err(DiacMiss { top, missing_text: Cow::Borrowed(missing_text) })
}

pub const fn pos(left: u32, bottom: u32, right: u32, top: u32) -> Result {
    Ok(Rect { left, bottom, right, top })
}
