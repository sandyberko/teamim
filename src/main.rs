mod fuzzy_find;
mod glyph;
mod leptonica_ext;
mod tesseract_ext;

use std::{
    ffi::{CString, OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{stdin, stdout, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;
use eyre::{bail, eyre, Context, ContextCompat, OptionExt};
use glyph::{with_glyph, Placement};
use leptess::leptonica::{self, BoxGeometry, Pix};
use leptonica_ext::PixExt;
use tesseract_ext::{BoundingBox, PageIteratorLevel, Tess};

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

    if let Some(corrected) = &args.corrected {
        if corrected.extension() != Some(OsStr::new("box")) {
            bail!("expected .box extension, found {corrected:?}");
        }
        let text = BufReader::new(File::open(corrected)?)
            .lines()
            .map(|line| line?.chars().next().ok_or_eyre("empty line"))
            .collect::<eyre::Result<String>>()?;
        let boxes = BufReader::new(File::open(corrected)?).lines().map(|line| {
            Ok(into_geometry(
                parse_box_line(line?)?,
                pix_h,
                OriginPos::TopLeft,
            ))
        });
        place_teamim(&pix, &text, boxes, &args)?;
    } else if let Some(write_boxes) = args.write_boxes {
        use tesseract_ext::{BoundingBox, Tess};
        let mut tess = Tess::new(c"./assets/tessdata", c"stam")?;

        tess.set_image(&pix);
        tess.recognize()?;

        if args.render_boxes {
            let boxes = tess.get_component_images(PageIteratorLevel::Symbol, true)?;
            pix.render_boxes(boxes, 2, (0, 255, 0))
                .map_err(|_| eyre!("failed to render boxes"))?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&write_boxes)?;

        let mut w = BufWriter::new(file);

        for char in tess.results_iter() {
            let text = char.text();
            let BoundingBox {
                left,
                bottom,
                right,
                top,
            } = char.bounding_box();
            writeln!(&mut w, "{text} {left} {bottom} {right} {top} 0")?;
        }

        w.flush()?;
        println!("Wrote to {write_boxes:?}");
    } else {
        let mut tess = Tess::new(c"./assets/tessdata", c"stam")?;

        tess.set_image(&pix);
        tess.recognize()?;

        // Print
        let text = tess.get_text()?;
        println!("=== Text ===");
        println!("{text}");
        println!("=== End Text ===");

        // Boxes
        let boxes = tess
            .results_iter()
            .map(|r| Ok(into_geometry(r.bounding_box(), pix_h, OriginPos::TopLeft)));
        place_teamim(&pix, text.as_str()?, boxes, &args)?;

        if args.render_boxes {
            let width = 2;
            let color = (0, 255, 0);
            let boxes = tess
                .get_component_images(PageIteratorLevel::Symbol, true)
                .wrap_err("no boxes")?;
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
    args: &Args,
) -> eyre::Result<()> {
    let consonants = fs::read_to_string("./assets/text/leningrad/consonants/torah.txt")?;
    let text = text.trim();
    let i =
        fuzzy_find::find(&consonants, text).wrap_err_with(|| format!("no match for {text:?}"))?;
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
                let Some(cur_c) = cur_c else {
                    bail!("expected char");
                };
                with_glyph(c_taam, |glyph| {
                    eprintln!("ta'am {} on {cur_c:?}", glyph.name);

                    const SCALE_FACTOR: f32 = 0.5;
                    let pix = glyph.pix.scale(SCALE_FACTOR)?;

                    const TOP_MARGIN: i32 = 7;
                    let (x, y) = match glyph.placement {
                        Placement::Top => (cur_box.x, cur_box.y - TOP_MARGIN - 7),
                        Placement::Bottom => (cur_box.x, cur_box.y + cur_box.h - TOP_MARGIN),
                    };

                    // debug
                    let w = pix.get_w().try_into().unwrap();
                    let h = pix.get_w().try_into().unwrap();
                    if args.render_boxes {
                        img.render_box(&BoxGeometry { x, y, w, h }, 2, (0, 0, 255))?;
                    }

                    // + h ???
                    img.render_img(&pix, x, y + h)
                })?
                .wrap_err("failed to render text")?;

                if args.render_boxes {
                    img.render_box(cur_box, 3, (0, 255, 0))
                        .wrap_err_with(|| format!("invalid box {cur_box:?} for {cur_c:?}"))?;
                }
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

                    if args.interactive {
                        println!("expected {c_taam:?}, but recognized {cur_c:?}");
                        print!(
                            "ignore and [c]ontinue, [s]top but save, [w]rite debug image, [q]uit: "
                        );
                        stdout().flush()?;
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

enum OriginPos {
    BottomLeft,
    TopLeft,
}

fn into_geometry(
    BoundingBox {
        left,
        bottom,
        right,
        top,
    }: BoundingBox,
    img_h: u32,
    origin_pos: OriginPos,
) -> BoxGeometry {
    let img_h = img_h as i32;
    match origin_pos {
        OriginPos::BottomLeft => BoxGeometry {
            x: left,
            y: img_h - top,
            w: right - left,
            h: top - bottom,
        },
        OriginPos::TopLeft => BoxGeometry {
            x: left,
            y: top,
            w: right - left,
            h: bottom - top,
        },
    }
}

fn parse_box_line(line: String) -> eyre::Result<BoundingBox> {
    let mut parts = line.split(' ');
    let _char = parts.next().ok_or_eyre("failed to parse char")?;
    Ok(BoundingBox {
        left: parts.next().ok_or_eyre("failed to parse left")?.parse()?,
        bottom: parts.next().ok_or_eyre("failed to parse bottom")?.parse()?,
        right: parts.next().ok_or_eyre("failed to parse right")?.parse()?,
        top: parts.next().ok_or_eyre("failed to parse top")?.parse()?,
    })
}
