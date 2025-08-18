use core::slice;
use std::{
    ffi::CStr,
    mem::MaybeUninit,
    ptr::{self, NonNull},
};

use eyre::{ContextCompat, OptionExt, bail, ensure};
use leptonica_sys::{
    PIX_DST, PIX_SRC, boxCreate, pixConvertTo32, pixRasterop, pixRenderBoxArb, pixRenderBoxaArb,
    pixScale, pixWriteAutoFormat,
};

#[derive(Debug, thiserror::Error)]
#[error("Generic Leptonica error")]
pub struct Error;

#[derive(Debug, Clone, Copy)]
pub struct BoxGeometry {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub struct PixBox(NonNull<leptonica_sys::Pix>);
impl Drop for PixBox {
    fn drop(&mut self) {
        let mut raw = self.as_mut_ptr();
        unsafe { leptonica_sys::pixDestroy(&raw mut raw) }
    }
}

impl Clone for PixBox {
    fn clone(&self) -> Self {
        let new = unsafe { leptonica_sys::pixCopy(ptr::null_mut(), self.as_ptr()) };
        NonNull::new(new).map(Self).unwrap()
    }
}

impl PixBox {
    // TODO private
    pub(crate) fn as_ptr(&self) -> *const leptonica_sys::Pix {
        self.0.as_ptr()
    }

    // TODO private
    pub(crate) fn as_mut_ptr(&mut self) -> *mut leptonica_sys::Pix {
        self.0.as_ptr()
    }

    pub fn read(filename: &CStr) -> eyre::Result<Self> {
        let ptr = unsafe { leptonica_sys::pixRead(filename.as_ptr()) };
        NonNull::new(ptr).ok_or_eyre("failed to read image").map(Self)
    }

    pub fn read_mem(img: &[u8]) -> eyre::Result<Self> {
        let ptr = unsafe { leptonica_sys::pixReadMem(img.as_ptr(), img.len()) };
        NonNull::new(ptr).ok_or_eyre("failed to read image").map(Self)
    }

    #[must_use]
    pub fn get_w(&self) -> i32 {
        unsafe { leptonica_sys::pixGetWidth(self.as_ptr()) }
    }

    #[must_use]
    pub fn get_h(&self) -> i32 {
        unsafe { leptonica_sys::pixGetHeight(self.as_ptr()) }
    }

    pub fn render_boxes(
        &mut self,
        mut boxes: Boxes,
        width: i32,
        (rval, gval, bval): (u8, u8, u8),
    ) -> eyre::Result<()> {
        let result = unsafe {
            pixRenderBoxaArb(self.as_mut_ptr(), boxes.as_mut_ptr(), width, rval, gval, bval)
        };
        if result == 0 { Ok(()) } else { bail!("Failed to render boxes") }
    }

    pub fn render_box(
        &mut self,
        &BoxGeometry { x, y, w, h }: &BoxGeometry,
        width: i32,
        (rval, gval, bval): (u8, u8, u8),
    ) -> Result<(), Error> {
        let r#box = unsafe { boxCreate(x, y, w, h) };
        let result = unsafe { pixRenderBoxArb(self.as_mut_ptr(), r#box, width, rval, gval, bval) };
        if result == 0 { Ok(()) } else { Err(Error) }
    }

    /// ⚠️ `y` should be the bottom of the `src` image!
    pub fn render_img(&mut self, mut src: PixBox, x: i32, y: i32) -> Result<(), eyre::Error> {
        let pix_dest = self.as_mut_ptr();
        let pix_source = src.as_mut_ptr();

        let y = y.checked_sub(src.get_h()).wrap_err("ta'am is over the edge")?;

        let result = unsafe {
            pixRasterop(
                pix_dest,
                x,
                y,
                src.get_w(),
                src.get_h(),
                (PIX_SRC & PIX_DST).try_into().unwrap(),
                pix_source,
                0,
                0,
            )
        };
        if result == 0 { Ok(()) } else { bail!("Failed to render image") }
    }

    pub fn write(&mut self, filename: &CStr) -> Result<(), eyre::Error> {
        let result = unsafe { pixWriteAutoFormat(filename.as_ptr(), self.as_mut_ptr()) };
        if result == 0 { Ok(()) } else { bail!("Failed to write. error code: {result}") }
    }

    pub fn into_32(&mut self) -> Result<PixBox, eyre::Error> {
        let result = unsafe { pixConvertTo32(self.as_mut_ptr()) };
        NonNull::new(result).ok_or_eyre("failed to convert to 32").map(Self)
    }

    pub fn scale(&mut self, factor: f32) -> eyre::Result<Self> {
        let result = unsafe { pixScale(self.as_mut_ptr(), factor, factor) };
        NonNull::new(result).ok_or_eyre("scale failed").map(Self)
    }

    pub fn copy_to_png(&mut self) -> eyre::Result<Buf> {
        let mut buf_ptr = MaybeUninit::uninit();
        let mut buf_size = MaybeUninit::uninit();
        let result = unsafe {
            leptonica_sys::pixWriteMemPng(
                buf_ptr.as_mut_ptr(),
                buf_size.as_mut_ptr(),
                self.as_mut_ptr(),
                0.0,
            )
        };
        if result != 0 {
            bail!("pixWriteMemPng failed");
        }
        let ptr = unsafe { buf_ptr.assume_init() };
        let len = unsafe { buf_size.assume_init() };
        Ok(Buf { ptr, len })
    }
    pub fn blur(&mut self, kernel_size: i32) -> eyre::Result<Self> {
        let pixd =
            unsafe { leptonica_sys::pixBlockconv(self.as_mut_ptr(), kernel_size, kernel_size) };
        NonNull::new(pixd).ok_or_eyre("failed to blur").map(Self)
    }

    pub fn contrast(&mut self, factor: f32) -> eyre::Result<()> {
        let result =
            unsafe { leptonica_sys::pixContrastTRC(self.as_mut_ptr(), self.as_mut_ptr(), factor) };
        ensure!(!result.is_null(), "Failed to contrast");
        Ok(())
    }
}

pub struct Buf {
    ptr: *mut u8,
    len: usize,
}
impl Drop for Buf {
    fn drop(&mut self) {
        unsafe { leptonica_sys::free(self.ptr.cast()) }
    }
}
impl AsRef<[u8]> for Buf {
    fn as_ref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}
// TODO check safety
unsafe impl Send for Buf {}

// TODO private
pub struct Boxes(pub(crate) NonNull<leptonica_sys::Boxa>);
impl Drop for Boxes {
    fn drop(&mut self) {
        let mut raw = self.0.as_ptr();
        unsafe { leptonica_sys::boxaDestroy(&raw mut raw) }
    }
}
impl Boxes {
    fn as_mut_ptr(&mut self) -> *mut leptonica_sys::Boxa {
        self.0.as_ptr()
    }
}
