use std::{
    ffi::CStr,
    marker::PhantomData,
    ptr::{self, NonNull},
};

use leptess::{
    capi::{
        TessBaseAPI, TessBaseAPICreate, TessBaseAPIDelete, TessBaseAPIEnd, TessBaseAPIGetIterator,
        TessBaseAPIInit3, TessBaseAPIRecognize, TessBaseAPISetImage2, TessPageIteratorBoundingBox,
        TessPageIteratorLevel_RIL_SYMBOL, TessResultIterator, TessResultIteratorGetUTF8Text,
        TessResultIteratorNext,
    },
    leptonica::{self, BoxGeometry},
};

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
    pub fn new(datapath: &CStr, language: &CStr) -> Self {
        let tess = Tess {
            raw: NonNull::new(unsafe { TessBaseAPICreate() }).unwrap(),
        };

        let err =
            unsafe { TessBaseAPIInit3(tess.raw.as_ptr(), datapath.as_ptr(), language.as_ptr()) };
        assert_eq!(err, 0);
        tess
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

    pub fn recognize(&mut self) {
        let err = unsafe { TessBaseAPIRecognize(self.raw.as_ptr(), ptr::null_mut()) };
        assert_eq!(err, 0);
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
    pub fn text<'s>(&mut self) -> &'s str {
        let cstr = unsafe { TessResultIteratorGetUTF8Text(self.raw.as_ptr(), self.level) };
        if cstr.is_null() {
            panic!("failed to get text");
        }
        unsafe { CStr::from_ptr(cstr) }.to_str().unwrap()
    }
    pub fn bounding_box(&mut self) -> BoundingBox {
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
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
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
