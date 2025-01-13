use std::{error::Error, ffi::CStr, mem};

use leptess::{
    capi::{pixConvertTo32, pixRasterop, pixRenderBoxaArb, pixWriteAutoFormat, PIX_DST, PIX_SRC},
    leptonica::{Boxa, Pix},
};
use leptonica_plumbing::memory::RefCounted;

pub trait PixExt {
    fn render_boxes(&mut self, boxes: Boxa, width: i32, color: (u8, u8, u8)) -> Result<(), ()>;
    fn render_img(&mut self, src: &mut Pix, x: i32, y: i32) -> Result<(), Box<dyn Error>>;
    fn write(&mut self, path: &CStr) -> Result<(), Box<dyn Error>>;
    fn convert_to_32(&mut self) -> Result<(), Box<dyn Error>>;
}

impl PixExt for Pix {
    fn render_boxes(
        &mut self,
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

    fn render_img(&mut self, src: &mut Pix, x: i32, y: i32) -> Result<(), Box<dyn Error>> {
        let pixd = *self.raw.as_ref();
        let pixs: *mut _ = *src.raw.as_ref();

        let y = y
            .checked_sub_unsigned(src.get_h())
            .ok_or("ta'am is over the edge")?;

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
            Err("Failed to render image".into())
        }
    }

    fn write(&mut self, filename: &CStr) -> Result<(), Box<dyn Error>> {
        let result = unsafe { pixWriteAutoFormat(filename.as_ptr(), *self.raw.as_ref()) };
        if result == 0 {
            Ok(())
        } else {
            Err(format!("Failed to write. error code: {result}").into())
        }
    }

    fn convert_to_32(&mut self) -> Result<(), Box<dyn Error>> {
        let result = unsafe { pixConvertTo32(*self.raw.as_ref()) };
        if result.is_null() {
            Err("Failed to convert to 32".into())
        } else {
            let plumbing_pix = unsafe { leptonica_plumbing::Pix::new_from_pointer(result) };
            let raw = unsafe { RefCounted::new(plumbing_pix) };
            let _ = mem::replace(&mut self.raw, raw);
            Ok(())
        }
    }
}
