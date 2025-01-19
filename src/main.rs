mod fuzzy_find;
mod glyphs;
mod leptonica_ext;
mod tesseract_ext;

use std::{
    ffi::{CString, OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{stdin, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;
use eyre::{bail, eyre, Context, ContextCompat, OptionExt};
use glyphs::with_glyph;
use leptess::{
    capi,
    leptonica::{self, BoxGeometry, Pix},
    tesseract::TessApi,
};
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

    #[clap(short, long)]
    corrected: Option<PathBuf>,

    #[clap(short, long)]
    write_boxes: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let mut pix = leptonica::pix_read(&args.input_image_name)?;
    let pix_h = pix.get_h();
    pix.convert_to_32()?;

    if let Some(corrected) = args.corrected {
        let text = fs::read_to_string(corrected.with_extension("txt"))?;
        let boxes = BufReader::new(File::open(corrected.with_extension("box"))?)
            .lines()
            .map(|line| parse_box_line(line?, pix_h));
        place_teamim(&pix, &text, boxes, args.interactive)?;
    } else if let Some(write_boxes) = args.write_boxes {
        use tesseract_ext::{Tess, BoundingBox};
        let mut tess = Tess::new(c"./assets/tessdata", c"stam");

        tess.set_image(&pix);
        tess.recognize();

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&write_boxes)?;

        let mut w = BufWriter::new(file);

        for mut char in tess.results_iter() {
            let text = char.text();
            let BoundingBox { left, top, right, bottom } = char.bounding_box();
            writeln!(&mut w, "{text} {left} {top} {right} {bottom} 0")?;
        }

        w.flush()?;
        println!("Wrote to {write_boxes:?}");
    } else {
        let mut tess = TessApi::new(Some("./assets/tessdata"), "stam")?;

        tess.set_image(&pix);

        // Print
        let text = tess.get_utf8_text()?;
        println!("=== Text ===");
        println!("{text}");
        println!("=== End Text ===");

        // Boxes
        let boxes = tess
            .get_component_images(capi::TessPageIteratorLevel_RIL_SYMBOL, true)
            .wrap_err("no boxes")?;

        place_teamim(
            &pix,
            &text,
            boxes.into_iter().map(|r#box| Ok(r#box.get_geometry())),
            args.interactive,
        )?;

        if args.render_boxes {
            let width = 2;
            let color = (0, 255, 0);
            pix.render_boxes(boxes, width, color)
                .map_err(|_| eyre!("failed to render boxes"))?;
        }
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

fn place_teamim(
    img: &Pix,
    text: &str,
    boxes: impl IntoIterator<Item = eyre::Result<BoxGeometry>>,
    interactive: bool,
) -> eyre::Result<()> {
    let consonants = fs::read_to_string("./assets/text/leningrad/consonants/torah.txt")?;
    let i = fuzzy_find::find(&consonants, text.trim()).wrap_err("no match")?;
    println!("Found at index {i}");

    let mut chars_iter = text.chars();
    let mut cur_c = None;
    let mut boxes_iter = boxes.into_iter();
    let mut cur_box: Option<BoxGeometry> = None;
    let teamim = fs::read_to_string("./assets/text/leningrad/teamim/torah.txt")?;
    'teamim: for c_taam in teamim.chars() {
        match c_taam {
            // Ta'am
            // TODO remove non-torah teamim
            '\u{0591}'..='\u{05AD}' | '\u{5bd}' => {
                let cur_box = cur_box.as_ref().ok_or_eyre("expected box")?;
                with_glyph(c_taam, |glyph, name| {
                    eprintln!("ta'am {name} on {cur_c:?}");
                    img.render_img(glyph, cur_box.x, cur_box.y)
                })?
                .wrap_err("failed to render text")?;
                img.render_box(cur_box, 3, (0, 255, 0))
                    .wrap_err_with(|| format!("invalid box {cur_box:?} for {cur_c:?}"))?;
            }
            // TODO Sof Pasuq, Maqaf, Paseq
            '\u{5c3}' | '\u{5be}' | '\u{5c0}' => (),
            '\n' => continue,
            // Niqqud
            ('\u{05b0}'..='\u{05bc}') | '\u{05c1}' | '\u{05c2}' => continue,
            _ if c_taam.is_whitespace() => continue,
            // Letter - alef to tav
            ('\u{05d0}'..='\u{05EA}') => {
                cur_c = chars_iter.find(|c| !c.is_whitespace());
                cur_box = boxes_iter.next().transpose()?;

                let cur_c = cur_c.ok_or_eyre("no text")?;
                'mismatch: {
                    if c_taam == cur_c {
                        break 'mismatch;
                    }

                    if interactive {
                        println!("expected {c_taam:?}, but recognized {cur_c:?}");
                        println!(
                            "ignore and [c]ontinue, [s]top but save, [w]rite debug image, [q]uit: "
                        );
                        let mut buf = String::new();
                        stdin().read_line(&mut buf)?;
                        match buf.trim() {
                            "c" => break 'mismatch,
                            "s" => break 'teamim,
                            "w" => {
                                img.render_box(&cur_box.ok_or_eyre("need box")?, 3, (255, 0, 0))?;
                                img.write(c"debug.jpg")?;
                            }
                            _ => {}
                        }
                    }

                    bail!("expected {c_taam:?}, but recognized {cur_c:?}");
                }
            }
            c => bail!("unexpected taaam_c: 0x{:x} {c:?}", c as u32),
        }
    }
    Ok(())
}

fn parse_box_line(line: String, img_h: u32) -> eyre::Result<BoxGeometry> {
    let mut parts = line.split(' ');
    let _char = parts.next().ok_or_eyre("failed to parse char")?;
    let left = parts.next().ok_or_eyre("failed to parse left")?.parse()?;
    let bottom: i32 = parts.next().ok_or_eyre("failed to parse bottom")?.parse()?;
    let right: i32 = parts.next().ok_or_eyre("failed to parse right")?.parse()?;
    let top: i32 = parts.next().ok_or_eyre("failed to parse top")?.parse()?;
    let img_h = img_h as i32;
    let w = right - left;
    let h = top - bottom;
    let x = left;
    let y = img_h - top;
    Ok(BoxGeometry { x, y, w, h })
}
