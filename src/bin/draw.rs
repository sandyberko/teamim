use core::slice;
use std::path::PathBuf;

use clap::Parser;
use eyre::OptionExt;
use imageproc::image::GrayImage;
use leptess::leptonica;

#[derive(Parser)]
struct Args {
    image: PathBuf,
    out: PathBuf,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();

    // Load a Pix image (example assumes you have a Pix already)
    let pix = leptonica::pix_read(&args.image)?;

    // Convert Pix to DynamicImage
    let width = pix.get_w();
    let height = pix.get_h();
    let depth = pix.raw.get_depth();
    // grayscale
    assert_eq!(depth, 8);
    let data = pix.raw.get_data();
    let data_size: usize = (width * height).try_into().unwrap();
    let data = unsafe { slice::from_raw_parts_mut(data as *mut u8, data_size) };
    let img = GrayImage::from_raw(width, height, data.to_vec()).ok_or_eyre("error")?;

    // Load a custom font (ensure the .ttf file exists)
    // let font = FontRef::try_from_slice(include_bytes!("../../assets/fonts/STAM.ttf"))?;

    // Draw text onto the image
    // imageproc::drawing::draw_text_mut(
    //     &mut img,
    //     Luma([255]), // Red color
    //     50,
    //     50,
    //     20.0,
    //     &font,
    //     "מה לעזאזל",
    // );

    img.save(args.out)?;

    Ok(())
}
