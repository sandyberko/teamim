use std::{ffi::CStr, mem};

use eyre::{bail, ContextCompat};
use leptess::{
    capi::{
        boxCreate, pixConvertTo32, pixRasterop, pixRenderBoxArb, pixRenderBoxaArb, pixScale,
        pixWriteAutoFormat, PIX_DST, PIX_SRC,
    },
    leptonica::{BoxGeometry, Boxa, Pix},
};
use leptonica_plumbing::memory::RefCounted;

#[derive(Debug, thiserror::Error)]
#[error("Generic Leptonica error")]
pub(crate) struct Error;

pub trait PixExt: Sized {
    fn render_box(&self, geom: &BoxGeometry, width: i32, color: (u8, u8, u8)) -> Result<(), Error>;
    fn render_boxes(&self, boxes: Boxa, width: i32, color: (u8, u8, u8)) -> Result<(), ()>;
    fn render_img(&self, src: &Pix, x: i32, y: i32) -> Result<(), eyre::Error>;
    fn write(&self, path: &CStr) -> Result<(), eyre::Error>;
    fn convert_to_32(&mut self) -> Result<(), eyre::Error>;
    fn scale(&self, factor: f32) -> eyre::Result<Self>;
}

impl PixExt for Pix {
    fn render_boxes(
        &self,
        mut boxes: Boxa,
        width: i32,
        (rval, gval, bval): (u8, u8, u8),
    ) -> Result<(), ()> {
        let result = unsafe {
            pixRenderBoxaArb(
                *self.raw.as_ref(),
                boxes.raw.as_mut(),
                width,
                rval,
                gval,
                bval,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(())
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
        let pixd = *self.raw.as_ref();
        let pixs: *mut _ = *src.raw.as_ref();

        let y = y
            .checked_sub_unsigned(src.get_h())
            .wrap_err("ta'am is over the edge")?;

        let result = unsafe {
            pixRasterop(
                pixd,
                x,
                y,
                src.get_w().try_into().unwrap(),
                src.get_h().try_into().unwrap(),
                (PIX_SRC & PIX_DST).try_into().unwrap(),
                pixs,
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
        } else {
            let plumbing_pix = unsafe { leptonica_plumbing::Pix::new_from_pointer(result) };
            let raw = unsafe { RefCounted::new(plumbing_pix) };
            let _ = mem::replace(&mut self.raw, raw);
            Ok(())
        }
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
}
