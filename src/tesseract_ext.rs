use std::{
    ffi::{CStr, c_char},
    fmt::Display,
    ptr::{self, NonNull},
};

use eyre::{OptionExt, bail};
use leptess::{capi, leptonica};

use crate::leptonica_ext::Boxes;

#[derive(Copy, Clone)]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[cfg_attr(target_os = "windows", repr(i32))]
pub enum PageIteratorLevel {
    Textline = capi::TessPageIteratorLevel_RIL_TEXTLINE,
    Symbol = capi::TessPageIteratorLevel_RIL_SYMBOL,
}

#[derive(Default)]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[cfg_attr(target_os = "windows", repr(i32))]
pub enum PageSegMode {
    #[default]
    Auto = capi::TessPageSegMode_PSM_AUTO,
    SingleColumn = capi::TessPageSegMode_PSM_SINGLE_COLUMN,
}

#[derive(Clone)]
pub struct Tess {
    raw: NonNull<capi::TessBaseAPI>,
}

// TODO are you sure?
unsafe impl Send for Tess {}

impl Drop for Tess {
    fn drop(&mut self) {
        unsafe { capi::TessBaseAPIDelete(self.raw.as_ptr()) }
    }
}

impl Tess {
    pub fn new(datapath: &CStr, language: &CStr) -> eyre::Result<Self> {
        let raw = NonNull::new(unsafe { capi::TessBaseAPICreate() })
            .ok_or_eyre("failed to create tesseract")?;

        let err =
            unsafe { capi::TessBaseAPIInit3(raw.as_ptr(), datapath.as_ptr(), language.as_ptr()) };

        if err != 0 {
            bail!("failed to init tesseract {err:x}");
        }
        Ok(Tess { raw })
    }

    pub fn set_variable(
        &mut self,
        name: impl AsRef<CStr>,
        value: impl AsRef<CStr>,
    ) -> eyre::Result<()> {
        let succeed = unsafe {
            capi::TessBaseAPISetVariable(
                self.raw.as_ptr(),
                name.as_ref().as_ptr(),
                value.as_ref().as_ptr(),
            )
        };
        if succeed != 1 {
            bail!("failed to set variable {succeed:x}");
        }
        Ok(())
    }

    pub fn results_iter(&mut self, level: PageIteratorLevel) -> ResultIter {
        let iter_ptr = unsafe { capi::TessBaseAPIGetIterator(self.raw.as_ptr()) };
        ResultIter {
            raw: NonNull::new(iter_ptr).unwrap(),
            level,
            is_first: true,
        }
    }

    pub fn set_image(&mut self, img: &leptonica::Pix) {
        unsafe { capi::TessBaseAPISetImage2(self.raw.as_ptr(), *img.raw.as_ref()) }
    }

    pub fn recognize(&mut self) -> eyre::Result<()> {
        let err = unsafe { capi::TessBaseAPIRecognize(self.raw.as_ptr(), ptr::null_mut()) };
        if err != 0 {
            bail!("failed to recognize: {err:x}");
        }
        Ok(())
    }

    pub fn get_component_images(
        &self,
        level: PageIteratorLevel,
        text_only: bool,
    ) -> eyre::Result<Boxes> {
        let ptr = unsafe {
            capi::TessBaseAPIGetComponentImages(
                self.raw.as_ptr(),
                level as _,
                text_only.into(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if ptr.is_null() {
            bail!("failed to get component images");
        } else {
            Ok(unsafe { Boxes::new(ptr) })
        }
    }

    pub fn get_text(&self) -> eyre::Result<Text> {
        let cstr = unsafe { capi::TessBaseAPIGetUTF8Text(self.raw.as_ptr()) };
        if cstr.is_null() {
            bail!("failed to get text");
        } else {
            Ok(Text(NonNull::new(cstr).ok_or_eyre("failed to get text")?))
        }
    }

    pub fn set_page_seg_mode(&self, mode: PageSegMode) {
        unsafe { capi::TessBaseAPISetPageSegMode(self.raw.as_ptr(), mode as _) }
    }
}

pub struct Text(NonNull<c_char>);

impl Text {
    pub fn as_str(&self) -> eyre::Result<&str> {
        Ok(unsafe { CStr::from_ptr(self.0.as_ptr()) }.to_str()?)
    }
}

impl Drop for Text {
    fn drop(&mut self) {
        unsafe { capi::TessDeleteText(self.0.as_ptr()) }
    }
}

impl Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = unsafe { CStr::from_ptr(self.0.as_ptr()) }
            .to_str()
            .map_err(|_| std::fmt::Error)?;
        f.write_str(str)
    }
}

pub struct ResultIter {
    raw: NonNull<capi::TessResultIterator>,
    level: PageIteratorLevel,
    is_first: bool,
}

impl Drop for ResultIter {
    fn drop(&mut self) {
        unsafe { capi::TessResultIteratorDelete(self.raw.as_ptr()) };
    }
}

impl Iterator for ResultIter {
    type Item = BoundingBox<Text>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_first {
            self.is_first = false;
        } else {
            let has_next =
                unsafe { capi::TessResultIteratorNext(self.raw.as_ptr(), self.level as _) };
            if has_next != 1 {
                return None;
            }
        }
        let bb = self.bounding_box();
        let text = self.text();
        Some(bb.with_value(text))
    }
}

impl ResultIter {
    #[must_use]
    pub fn text(&self) -> Text {
        let cstr =
            unsafe { capi::TessResultIteratorGetUTF8Text(self.raw.as_ptr(), self.level as _) };
        let cstr = NonNull::new(cstr).expect("failed to get text");
        Text(cstr)
    }
    #[must_use]
    pub fn bounding_box(&self) -> BoundingBox<()> {
        let mut r#box = BoundingBox::default();
        let err = unsafe {
            capi::TessPageIteratorBoundingBox(
                self.raw.as_ptr() as _,
                self.level as _,
                &mut r#box.left,
                &mut r#box.top,
                &mut r#box.right,
                &mut r#box.bottom,
            )
        };
        assert_eq!(err, 1);
        r#box
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundingBox<Value> {
    pub value: Value,

    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

impl<V: Default> Default for BoundingBox<V> {
    fn default() -> Self {
        Self {
            value: V::default(),
            left: -1,
            bottom: -1,
            right: -1,
            top: -1,
        }
    }
}

impl<V: Display> Display for BoundingBox<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            value: char,
            left,
            bottom,
            right,
            top,
        } = self;
        write!(f, "{char} {left} {bottom} {right} {top} 0")
    }
}

impl<V> BoundingBox<V> {
    pub fn new(value: V, left: i32, bottom: i32, right: i32, top: i32) -> Self {
        Self {
            value,
            left,
            bottom,
            right,
            top,
        }
    }

    #[must_use]
    pub fn with_value<O>(&self, value: O) -> BoundingBox<O> {
        BoundingBox {
            value,
            left: self.left,
            bottom: self.bottom,
            right: self.right,
            top: self.top,
        }
    }
    #[must_use]
    pub fn into_bottom_left(self, img_h: i32) -> Self {
        Self {
            value: self.value,
            left: self.left,
            bottom: img_h - self.bottom,
            right: self.right,
            top: img_h - self.top,
        }
    }
}
