mod leptonica_ext;

use std::{
    error::Error,
    ffi::{CString, OsStr, OsString},
    path::{Path, PathBuf},
};

use clap::Parser;
use leptess::{capi, leptonica, tesseract::TessApi};
use leptonica_ext::PixExt;

#[derive(Parser)]
struct Args {
    input_image_name: PathBuf,
    #[clap(short, long)]
    print: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut pix = leptonica::pix_read(&args.input_image_name)?;
    pix.convert_to_32()?;

    let mut tess = TessApi::new(Some("./assets/tessdata"), "stam")?;
    tess.set_image(&pix);

    if args.print {
        println!("{}", tess.get_utf8_text()?);
    }

    let boxes = tess
        .get_component_images(capi::TessPageIteratorLevel_RIL_SYMBOL, true)
        .ok_or("no boxes")?;

    let mut shalshelet = leptonica::pix_read(Path::new("./assets/glyphs/shalshelet.tif")).unwrap();

    for b in &boxes {
        let geometry = b.get_geometry();
        pix.render_img(&mut shalshelet, geometry.x, geometry.y)
            .map_err(|_| "failed to render text")?;
    }

    // let width = 2;
    // let color = (0, 255, 0);
    // pix.render_boxes(boxes, width, color)
    //     .map_err(|_| "failed to render boxes")?;

    let out_file_name = args.input_image_name.with_file_name(
        [
            args.input_image_name.file_stem().unwrap(),
            OsStr::new(".out."),
            args.input_image_name.extension().unwrap(),
        ]
        .into_iter()
        .collect::<OsString>(),
    );
    println!("Writing to {:?}", out_file_name);
    pix.write(&CString::new(out_file_name.into_os_string().into_encoded_bytes()).unwrap())?;
    Ok(())
}
