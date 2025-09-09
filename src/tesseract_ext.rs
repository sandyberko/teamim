pub mod bounding_box;

use core::fmt;
use std::{
    ffi::{CStr, c_char},
    fmt::{Debug, Display},
    mem::MaybeUninit,
    ptr::{self, NonNull},
};

use bounding_box::{BoundingBox, Rect};
use eyre::{OptionExt, bail};
use tesseract_sys as capi;

use crate::leptonica_ext::{Boxes, PixBox};

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
    SingleBlock = capi::TessPageSegMode_PSM_SINGLE_BLOCK,
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
        ResultIter { raw: NonNull::new(iter_ptr).unwrap(), level, is_first: true }
    }

    pub fn set_image(&mut self, img: &mut PixBox) {
        unsafe { capi::TessBaseAPISetImage2(self.raw.as_ptr(), img.as_mut_ptr()) }
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
        NonNull::new(ptr).ok_or_eyre("failed to get component images").map(Boxes)
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str().map_err(|_| fmt::Error)?)
    }
}

impl Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str().map_err(|_| fmt::Error)?)
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

        let rect = self.rect();
        let value = self.text();
        Some(BoundingBox { value, rect, page: 0 })
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
    pub fn rect(&self) -> Rect {
        let mut rect = MaybeUninit::<Rect>::uninit();
        let ptr = rect.as_mut_ptr();

        let succeed = unsafe {
            capi::TessPageIteratorBoundingBox(
                self.raw.as_ptr() as _,
                self.level as _,
                &raw mut (*ptr).left,
                &raw mut (*ptr).top,
                &raw mut (*ptr).right,
                &raw mut (*ptr).bottom,
            )
        };
        assert_eq!(succeed, 1);
        unsafe { rect.assume_init() }
    }
}
