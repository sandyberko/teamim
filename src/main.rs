mod fuzzy_find;
mod leptonica_ext;

use std::{
    ffi::{CString, OsStr, OsString},
    fs,
    io::stdin,
    path::{Path, PathBuf},
};

use clap::Parser;
use eyre::{bail, eyre, Context, ContextCompat, OptionExt};
use leptess::{capi, leptonica, tesseract::TessApi};
use leptonica_ext::PixExt;

#[derive(Parser)]
struct Args {
    input_image_name: PathBuf,
    #[clap(short, long)]
    print: bool,

    #[clap(short('b'), long)]
    render_boxes: bool,

    #[clap(short, long)]
    interactive: bool,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let mut pix = leptonica::pix_read(&args.input_image_name)?;
    pix.convert_to_32()?;

    let mut tess = TessApi::new(Some("./assets/tessdata"), "stam")?;
    tess.set_image(&pix);

    // Print

    let text = tess.get_utf8_text()?;
    println!("=== Text ===");
    println!("{text}");
    println!("=== End Text ===");

    let consonants = fs::read_to_string("./assets/text/leningrad/consonants/torah.txt")?;
    let i = fuzzy_find::find(&consonants, text.trim()).wrap_err("no match")?;
    println!("Found at index {i}");

    let mut text = text.chars().peekable();
    let teamim = fs::read_to_string("./assets/text/leningrad/teamim/torah.txt")?;

    // Boxes
    let boxes = tess
        .get_component_images(capi::TessPageIteratorLevel_RIL_SYMBOL, true)
        .wrap_err("no boxes")?;
    let mut boxes_iter = boxes.into_iter().peekable();

    // load glyphs
    let mut shalshelet = leptonica::pix_read(Path::new("./assets/glyphs/shalshelet.tif")).unwrap();

    'teamim: for c_taam in teamim.chars() {
        match c_taam {
            // Ta'am
            // TODO remove non-torah teamim like tzinor
            '\u{0591}'..='\u{05AE}' | '\u{5bd}' => {
                eprintln!("ta'am on {:?}", text.peek());
                let last_box = boxes_iter.peek().ok_or_eyre("expected box")?;
                let geometry = last_box.get_geometry();
                pix.render_img(&mut shalshelet, geometry.x, geometry.y)
                    .wrap_err("failed to render text")?;
                pix.render_box(last_box, 3, (255, 0, 0))?;
            }
            // TODO Sof Pasuq, Maqaf
            '\u{5c3}' | '\u{5be}' => (),
            '\n' => continue,
            // Niqqud
            ('\u{05b0}'..='\u{05bc}') | '\u{05c1}' | '\u{05c2}' => continue,
            _ if c_taam.is_whitespace() => continue,
            // Letter - alef to tav
            ('\u{05d0}'..='\u{05EA}') => {
                let c = loop {
                    match text.peek() {
                        Some(c) if c.is_whitespace() => {
                            text.next();
                        }
                        Some(c) => break *c,
                        None => bail!("no more text"),
                    }
                };
                boxes_iter.next();
                'mismatch: {
                    if c_taam == c {
                        break 'mismatch;
                    }

                    if args.interactive {
                        println!("expected {c_taam:?}, but recognized {c:?}");
                        println!(
                            "ignore and [c]ontinue, [s]top but save, [w]rite debug image, [q]uit: "
                        );
                        let mut buf = String::new();
                        stdin().read_line(&mut buf)?;
                        match buf.trim() {
                            "c" => break 'mismatch,
                            "s" => break 'teamim,
                            "w" => {
                                pix.render_box(
                                    boxes_iter.peek().ok_or_eyre("need box")?,
                                    3,
                                    (255, 0, 0),
                                )?;
                                pix.write(c"debug.jpg")?;
                            }
                            _ => {}
                        }
                    }

                    bail!("expected {c_taam:?}, but recognized {c:?}");
                }
                text.next();
            }
            c => bail!("unexpected taaam_c: 0x{:x} {c:?}", c as u32),
        }
    }

    if args.render_boxes {
        let width = 2;
        let color = (0, 255, 0);
        pix.render_boxes(boxes, width, color)
            .map_err(|_| eyre!("failed to render boxes"))?;
    }

    // Output image
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
