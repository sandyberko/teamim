use core::slice;
use std::{
    ffi::CStr,
    mem::{self, MaybeUninit},
};

use eyre::{bail, ContextCompat};
use leptess::{
    capi::{
        boxCreate, pixConvertTo32, pixRasterop, pixRenderBoxArb, pixRenderBoxaArb, pixScale,
        pixWriteAutoFormat, PIX_DST, PIX_SRC,
    },
    leptonica::{BoxGeometry, Pix},
};
use leptonica_plumbing::memory::RefCounted;

#[derive(Debug, thiserror::Error)]
#[error("Generic Leptonica error")]
pub struct Error;

pub trait PixExt: Sized {
    fn render_box(&self, geom: &BoxGeometry, width: i32, color: (u8, u8, u8)) -> Result<(), Error>;
    fn render_boxes(&self, boxes: Boxes, width: i32, color: (u8, u8, u8)) -> eyre::Result<()>;
    fn render_img(&self, src: &Pix, x: i32, y: i32) -> Result<(), eyre::Error>;
    fn write(&self, path: &CStr) -> Result<(), eyre::Error>;
    fn convert_to_32(&mut self) -> Result<(), eyre::Error>;
    fn scale(&self, factor: f32) -> eyre::Result<Self>;
    fn copy_to_png(&self) -> eyre::Result<Buf>;
}

impl PixExt for Pix {
    fn render_boxes(
        &self,
        boxes: Boxes,
        width: i32,
        (rval, gval, bval): (u8, u8, u8),
    ) -> eyre::Result<()> {
        let result =
            unsafe { pixRenderBoxaArb(*self.raw.as_ref(), boxes.0, width, rval, gval, bval) };
        if result == 0 {
            Ok(())
        } else {
            bail!("Failed to render boxes")
        }
    }

    fn render_box(
        &self,
        &BoxGeometry { x, y, w, h }: &BoxGeometry,
        width: i32,
        (rval, gval, bval): (u8, u8, u8),
    ) -> Result<(), Error> {
        let r#box = unsafe { boxCreate(x, y, w, h) };
        let result = unsafe { pixRenderBoxArb(*self.raw.as_ref(), r#box, width, rval, gval, bval) };
        if result == 0 {
            Ok(())
        } else {
            Err(Error)
        }
    }

    /// ⚠️ `y` should be the bottom of the `src` image!
    fn render_img(&self, src: &Pix, x: i32, y: i32) -> Result<(), eyre::Error> {
        let pix_dest = *self.raw.as_ref();
        let pix_source: *mut _ = *src.raw.as_ref();

        let y = y
            .checked_sub_unsigned(src.get_h())
            .wrap_err("ta'am is over the edge")?;

        let result = unsafe {
            pixRasterop(
                pix_dest,
                x,
                y,
                src.get_w().try_into().unwrap(),
                src.get_h().try_into().unwrap(),
                (PIX_SRC & PIX_DST).try_into().unwrap(),
                pix_source,
                0,
                0,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            bail!("Failed to render image")
        }
    }

    fn write(&self, filename: &CStr) -> Result<(), eyre::Error> {
        let result = unsafe { pixWriteAutoFormat(filename.as_ptr(), *self.raw.as_ref()) };
        if result == 0 {
            Ok(())
        } else {
            bail!("Failed to write. error code: {result}")
        }
    }

    fn convert_to_32(&mut self) -> Result<(), eyre::Error> {
        let result = unsafe { pixConvertTo32(*self.raw.as_ref()) };
        if result.is_null() {
            bail!("Failed to convert to 32")
        }

        let plumbing_pix = unsafe { leptonica_plumbing::Pix::new_from_pointer(result) };
        let raw = unsafe { RefCounted::new(plumbing_pix) };
        let _ = mem::replace(&mut self.raw, raw);
        Ok(())
    }

    fn scale(&self, factor: f32) -> eyre::Result<Self> {
        let result = unsafe { pixScale(*self.raw.as_ref(), factor, factor) };
        if result.is_null() {
            bail!("scale failed");
        }

        let plumbing_pix = unsafe { leptonica_plumbing::Pix::new_from_pointer(result) };
        let raw = unsafe { RefCounted::new(plumbing_pix) };
        Ok(Self { raw })
    }

    fn copy_to_png(&self) -> eyre::Result<Buf> {
        let mut buf_ptr = MaybeUninit::uninit();
        let mut buf_size = MaybeUninit::uninit();
        let result = unsafe {
            capi::pixWriteMemPng(
                buf_ptr.as_mut_ptr(),
                buf_size.as_mut_ptr(),
                *self.raw.as_ref(),
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
}

pub struct Buf {
    ptr: *mut u8,
    len: usize,
}
impl Drop for Buf {
    fn drop(&mut self) {
        unsafe { capi::free(self.ptr.cast()) }
    }
}
impl AsRef<[u8]> for Buf {
    fn as_ref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}
// TODO check safety
unsafe impl Send for Buf {}
// custom bindings

use leptess::capi;

pub struct Boxes(*mut capi::Boxa);

impl Drop for Boxes {
    fn drop(&mut self) {
        unsafe { capi::boxaDestroy(&raw mut self.0) }
    }
}

impl Boxes {
    pub unsafe fn new(ptr: *mut capi::Boxa) -> Self {
        Self(ptr)
    }
}
