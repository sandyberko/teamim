use std::{
    ffi::{c_char, CStr},
    fmt::Display,
    marker::PhantomData,
    ptr::{self, NonNull},
};

use eyre::{bail, OptionExt};
use leptess::{
    capi::{
        TessBaseAPI, TessBaseAPICreate, TessBaseAPIDelete, TessBaseAPIEnd,
        TessBaseAPIGetComponentImages, TessBaseAPIGetIterator, TessBaseAPIGetUTF8Text,
        TessBaseAPIInit3, TessBaseAPIRecognize, TessBaseAPISetImage2, TessPageIteratorBoundingBox,
        TessPageIteratorLevel_RIL_SYMBOL, TessResultIterator, TessResultIteratorGetUTF8Text,
        TessResultIteratorNext,
    },
    leptonica,
};

use crate::leptonica_ext::Boxes;

#[repr(i32)]
pub enum PageIteratorLevel {
    Symbol = TessPageIteratorLevel_RIL_SYMBOL,
}

pub struct Tess {
    raw: NonNull<TessBaseAPI>,
}

impl Drop for Tess {
    fn drop(&mut self) {
        unsafe { TessBaseAPIEnd(self.raw.as_ptr()) }
        unsafe { TessBaseAPIDelete(self.raw.as_ptr()) }
    }
}

impl Tess {
    pub fn new(datapath: &CStr, language: &CStr) -> eyre::Result<Self> {
        let raw = NonNull::new(unsafe { TessBaseAPICreate() })
            .ok_or_eyre("failed to create tesseract")?;

        let err = unsafe { TessBaseAPIInit3(raw.as_ptr(), datapath.as_ptr(), language.as_ptr()) };

        if err != 0 {
            bail!("failed to init tesseract {err:x}");
        }
        Ok(Tess { raw })
    }

    pub fn results_iter(&mut self) -> ResultIter {
        let iter_ptr = unsafe { TessBaseAPIGetIterator(self.raw.as_ptr()) };
        ResultIter {
            raw: NonNull::new(iter_ptr).unwrap(),
            level: TessPageIteratorLevel_RIL_SYMBOL,
            is_first: true,
            __phantom: PhantomData,
        }
    }

    pub fn set_image(&mut self, img: &leptonica::Pix) {
        unsafe { TessBaseAPISetImage2(self.raw.as_ptr(), *img.raw.as_ref()) }
    }

    pub fn recognize(&mut self) -> eyre::Result<()> {
        let err = unsafe { TessBaseAPIRecognize(self.raw.as_ptr(), ptr::null_mut()) };
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
            TessBaseAPIGetComponentImages(
                self.raw.as_ptr(),
                level as _,
                text_only as _,
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
        let cstr = unsafe { TessBaseAPIGetUTF8Text(self.raw.as_ptr()) };
        if cstr.is_null() {
            bail!("failed to get text");
        } else {
            Ok(Text(NonNull::new(cstr).ok_or_eyre("failed to get text")?))
        }
    }
}

pub struct Text(NonNull<c_char>);

impl Text {
    pub fn as_str(&self) -> eyre::Result<&str> {
        Ok(unsafe { CStr::from_ptr(self.0.as_ptr()) }.to_str()?)
    }
}

impl Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = unsafe { CStr::from_ptr(self.0.as_ptr()) }
            .to_str()
            .map_err(|_| std::fmt::Error)?;
        write!(f, "{str}")
    }
}

pub struct ResultIter<'tess> {
    raw: NonNull<TessResultIterator>,
    level: i32,
    is_first: bool,
    __phantom: PhantomData<&'tess mut TessResultIterator>,
}

impl<'tess> Iterator for ResultIter<'tess> {
    type Item = ResultItem<'tess>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.is_first {
            let has_next = unsafe { TessResultIteratorNext(self.raw.as_ptr(), self.level) };
            if has_next != 1 {
                return None;
            }
        } else {
            self.is_first = false;
        }
        Some(ResultItem {
            raw: self.raw,
            level: self.level,
            __phantom: PhantomData,
        })
    }
}

pub struct ResultItem<'tess> {
    raw: NonNull<TessResultIterator>,
    level: i32,
    __phantom: PhantomData<&'tess mut TessResultIterator>,
}

impl ResultItem<'_> {
    pub fn text<'s>(&self) -> &'s str {
        let cstr = unsafe { TessResultIteratorGetUTF8Text(self.raw.as_ptr(), self.level) };
        if cstr.is_null() {
            panic!("failed to get text");
        }
        unsafe { CStr::from_ptr(cstr) }.to_str().unwrap()
    }
    pub fn bounding_box(&self) -> BoundingBox {
        let mut r#box = BoundingBox::default();
        let err = unsafe {
            TessPageIteratorBoundingBox(
                self.raw.as_ptr() as _,
                self.level,
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

pub struct BoundingBox {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            left: -1,
            bottom: -1,
            right: -1,
            top: -1,
        }
    }
}
