mod fuzzy_find;
mod leptonica_ext;
mod tesseract_ext;

use std::{
    ffi::{CString, OsStr},
    fs::{self, File, OpenOptions},
    io::{stdin, stdout, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;
use eyre::{bail, eyre, Context, ContextCompat, OptionExt};
use leptess::leptonica::{self, BoxGeometry, Pix};
use leptonica_ext::PixExt;
use teamim::glyph::{Placement, GLYPHS};
use tesseract_ext::{BoundingBox, PageIteratorLevel, Tess};

#[derive(Parser)]
struct Args {
    /// Input image file
    input: PathBuf,

    /// Corrected box file
    #[clap(short, long)]
    corrected: Option<PathBuf>,

    /// Output image file name
    #[clap(short, long)]
    output: PathBuf,

    /// Write recognized boxes to file
    #[clap(short, long)]
    write_boxes: Option<PathBuf>,

    #[clap(flatten)]
    render: RenderArgs,

    /// Print recognized text
    #[clap(short, long)]
    print: bool,

    #[clap(short, long)]
    interactive: bool,
}

#[derive(Parser)]
struct RenderArgs {
    /// Render boxes to image
    #[clap(short('b'), long)]
    debug_boxes: bool,

    /// Render `Placement::After` te'amim like maqaf
    #[clap(long)]
    enable_after: bool,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let mut pix = leptonica::pix_read(&args.input)?;
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
                parse_box_line(&line?)?,
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

        if args.render.debug_boxes {
            let boxes = tess.get_component_images(PageIteratorLevel::Symbol, true)?;
            pix.render_boxes(boxes, 2, (0, 255, 0))?;
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

        if args.render.debug_boxes {
            let width = 2;
            let color = (0, 255, 0);
            let boxes = tess
                .get_component_images(PageIteratorLevel::Symbol, true)
                .wrap_err("no boxes")?;
            pix.render_boxes(boxes, width, color)
                .map_err(|_| eyre!("failed to render boxes"))?;
        }
    }

    println!("Writing to {:?}", args.output);
    let output = CString::new(args.output.into_os_string().into_encoded_bytes())?;
    pix.write(&output)?;
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
    let mut cur_line = 0usize;
    let mut cur_col = 0usize;
    let mut boxes_iter = boxes.into_iter();
    let mut cur_box: Option<BoxGeometry> = None;
    let teamim = fs::read_to_string("./assets/text/leningrad/teamim/torah.txt")?;
    let mut teamim_iter = teamim.char_indices();
    'teamim: while let Some((i_taam, c_taam)) = teamim_iter.next() {
        match c_taam {
            // Ta'am
            '\u{0591}'..='\u{05AD}' | '\u{5bd}' | '\u{5be}' | '\u{5c0}' | '\u{5c3}' => place_taam(
                img,
                args,
                cur_c.ok_or_eyre("expected char")?,
                cur_box.as_ref().ok_or_eyre("expected box")?,
                c_taam,
            )?,
            '\n' => {
                cur_line += 1;
                cur_col = 0;
                continue;
            }
            // Niqqud
            ('\u{05b0}'..='\u{05bc}') | '\u{05c1}' | '\u{05c2}' => continue,
            _ if c_taam.is_whitespace() => continue,
            // Letter - alef to tav
            ('\u{05d0}'..='\u{05EA}') => {
                cur_col += 1;
                cur_c = chars_iter.find(|c| !c.is_whitespace());
                cur_box = boxes_iter.next().transpose()?;

                let Some(cur_c) = cur_c else { break 'teamim };
                'mismatch: {
                    if c_taam == cur_c {
                        break 'mismatch;
                    }

                    let message = {
                        let i_iter = teamim_iter.clone().map(|(i, _)| i);
                        let start = teamim[..i_taam]
                            .char_indices()
                            .rev()
                            .skip(40)
                            .find(|(_, c)| c.is_whitespace())
                            .map_or(0, |(i, _)| i);
                        let before = &teamim[start..i_taam];

                        let next = i_iter.clone().next().unwrap_or(teamim.len());
                        let end = i_iter.clone().nth(30).unwrap_or(next);
                        let after = &teamim[next..end];
                        format!(
                            "expected {c_taam:?}, but recognized {cur_c:?} at {cur_line}:{cur_col}:\n\
                            ... {before}[{c_taam}]{after} ..."
                        )
                    };

                    if args.interactive {
                        println!("{message}");
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

                    bail!("{message}");
                }
            }
            c => bail!(
                "unexpected taaam_c: 0x{:x} {c:?} at {cur_line}:{cur_col}",
                c as u32
            ),
        }
    }
    Ok(())
}

fn place_taam(
    img: &Pix,
    args: &Args,
    cur_c: char,
    cur_box: &BoxGeometry,
    c_taam: char,
) -> Result<(), eyre::Error> {
    let Some(glyph) = GLYPHS.get(&c_taam) else {
        bail!("no glyph for {c_taam:?} {:x}", c_taam as u32);
    };

    if !args.render.enable_after && glyph.placement == Placement::After {
        return Ok(());
    }

    glyph
        .pix
        .try_with(|pix| {
            eprintln!(
                "ta'am {} on {cur_c:?}, placed {:?}",
                glyph.name, glyph.placement
            );

            let scale_factor = if glyph.placement == Placement::After {
                0.4
            } else {
                0.5
            };
            let pix = pix.scale(scale_factor)?;

            let top_margin = 4;
            let (x, y) = match glyph.placement {
                Placement::Top => (cur_box.x, cur_box.y - top_margin - 9),
                Placement::Bottom => (cur_box.x, cur_box.y + cur_box.h - top_margin),
                Placement::After => (cur_box.x - cur_box.w - 5, cur_box.y - top_margin),
            };

            // debug
            let w = pix.get_w().try_into().unwrap();
            let h = pix.get_w().try_into().unwrap();
            if args.render.debug_boxes {
                img.render_box(&BoxGeometry { x, y, w, h }, 2, (0, 0, 255))?;
            }

            // + h ???
            img.render_img(&pix, x, y + h)
        })?
        .wrap_err("failed to render text")?;
    if args.render.debug_boxes {
        img.render_box(cur_box, 3, (0, 255, 0))
            .wrap_err_with(|| format!("invalid box {cur_box:?} for {cur_c:?}"))?;
    }
    Ok(())
}

#[derive(Copy, Clone, Debug)]
enum OriginPos {
    #[expect(unused)]
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
    let img_h: i32 = img_h.try_into().unwrap();
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

fn parse_box_line(line: &str) -> eyre::Result<BoundingBox> {
    let mut parts = line.split(' ');
    let _char = parts.next().ok_or_eyre("failed to parse char")?;
    Ok(BoundingBox {
        left: parts.next().ok_or_eyre("failed to parse left")?.parse()?,
        bottom: parts.next().ok_or_eyre("failed to parse bottom")?.parse()?,
        right: parts.next().ok_or_eyre("failed to parse right")?.parse()?,
        top: parts.next().ok_or_eyre("failed to parse top")?.parse()?,
    })
}
