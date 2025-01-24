pub mod fuzzy_find;
pub mod glyph;
pub mod leptonica_ext;
pub mod tesseract_ext;

use std::{fmt::Write, fs};

use eyre::{bail, eyre, Context, OptionExt};
use glyph::{Placement, GLYPHS};
use leptess::leptonica::{self, BoxGeometry, Pix};
use leptonica_ext::PixExt;
use tesseract_ext::{BoundingBox, Tess};
use thiserror::Error;

pub fn recognize(img: &[u8]) -> eyre::Result<String> {
    let mut tess = Tess::new(c"./assets/tessdata", c"stam")?;

    let mut pix = leptonica::pix_read_mem(img)?;
    pix.convert_to_32()?;

    tess.set_image(&pix);
    tess.recognize()?;

    let mut w = String::new();
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

    Ok(w)
}

#[derive(Clone, Copy, Default)]
pub struct PlaceOptions {
    enable_after: bool,
    debug_boxes: bool,
}

#[derive(Error, Debug)]
#[error("Mismatch at box {box_number}")]
pub struct MismatchError {
    pub box_number: usize,
    pub expected: char,
}

impl MismatchError {
    pub fn message(
        &self,
        teamim: &str,
        cur_c: char,
        (i_taam, c_taam): (usize, char),
        teamim_iter: &(impl Iterator<Item = (usize, char)> + Clone),
    ) -> String {
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
            "expected {c_taam:?}, but recognized {cur_c:?}:\n\
                ... {before}[{c_taam}]{after} ..."
        )
    }
}

#[derive(Error, Debug)]
pub enum PlaceError {
    #[error("Not found")]
    NotFound,
    #[error("Mismatch at box {0}")]
    Mismatch(MismatchError),
    #[error(transparent)]
    Other(#[from] eyre::Report),
}

pub fn place_teamim(
    img: &(),
    options: PlaceOptions,
    text: &str,
    boxes: impl IntoIterator<Item = eyre::Result<BoxGeometry>>,
) -> Result<(), PlaceError> {
    let consonants = fs::read_to_string("./assets/text/leningrad/consonants/torah.txt")
        .wrap_err("failed to read consonants")?;
    let text = text.trim();
    let n = fuzzy_find::find(&consonants, text).ok_or(PlaceError::NotFound)?;

    let mut chars_iter = text.chars().enumerate();
    let mut cur_c: Option<(usize, char)> = None;
    let mut cur_line = 0usize;
    let mut cur_col = 0usize;
    let mut boxes_iter = boxes.into_iter();
    let mut cur_box: Option<BoxGeometry> = None;
    let teamim = fs::read_to_string("./assets/text/leningrad/teamim/torah.txt")
        .wrap_err("failed to read teamim")?;
    'teamim: for (_, c_taam) in teamim.char_indices().skip(n) {
        match c_taam {
            // Ta'am
            '\u{0591}'..='\u{05AD}' | '\u{5bd}' | '\u{5be}' | '\u{5c0}' | '\u{5c3}' => {
                let (_, cur_c) = cur_c.ok_or_eyre("expected char")?;
                let cur_box = cur_box.as_ref().ok_or_eyre("expected box")?;
                // place_taam(img, options, cur_c, cur_box, c_taam)?;
            }
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
                cur_c = chars_iter.find(|(_, c)| !c.is_whitespace());
                cur_box = boxes_iter.next().transpose()?;

                let Some((box_number, cur_c)) = cur_c else {
                    break 'teamim;
                };
                'mismatch: {
                    if c_taam == cur_c {
                        break 'mismatch;
                    }

                    return Err(PlaceError::Mismatch(MismatchError {
                        box_number,
                        expected: c_taam,
                    }));
                }
            }
            c => {
                return Err(eyre!(
                    "unexpected taaam_c: 0x{:x} {c:?} at {cur_line}:{cur_col}",
                    c as u32
                )
                .into())
            }
        }
    }
    Ok(())
}

fn place_taam(
    img: &Pix,
    options: PlaceOptions,
    cur_c: char,
    cur_box: &BoxGeometry,
    c_taam: char,
) -> Result<(), eyre::Error> {
    let Some(glyph) = GLYPHS.get(&c_taam) else {
        bail!("no glyph for {c_taam:?} {:x}", c_taam as u32);
    };

    if !options.enable_after && glyph.placement == Placement::After {
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
            if options.debug_boxes {
                img.render_box(&BoxGeometry { x, y, w, h }, 2, (0, 0, 255))?;
            }

            // + h ???
            img.render_img(&pix, x, y + h)
        })?
        .wrap_err("failed to render text")?;
    if options.debug_boxes {
        img.render_box(cur_box, 3, (0, 255, 0))
            .wrap_err_with(|| format!("invalid box {cur_box:?} for {cur_c:?}"))?;
    }
    Ok(())
}

pub fn parse_box_line(line: &str) -> eyre::Result<BoundingBox> {
    let mut parts = line.split(' ');
    let _char = parts.next().ok_or_eyre("failed to parse char")?;
    Ok(BoundingBox {
        left: parts.next().ok_or_eyre("failed to parse left")?.parse()?,
        bottom: parts.next().ok_or_eyre("failed to parse bottom")?.parse()?,
        right: parts.next().ok_or_eyre("failed to parse right")?.parse()?,
        top: parts.next().ok_or_eyre("failed to parse top")?.parse()?,
    })
}

#[derive(Copy, Clone, Debug)]
pub enum OriginPos {
    BottomLeft { img_h: u32 },
    TopLeft,
}

#[must_use]
pub fn into_geometry(
    BoundingBox {
        left,
        bottom,
        right,
        top,
    }: BoundingBox,
    origin_pos: OriginPos,
) -> BoxGeometry {
    match origin_pos {
        OriginPos::BottomLeft { img_h } => {
            let img_h: i32 = img_h.try_into().unwrap();
            BoxGeometry {
                x: left,
                y: img_h - top,
                w: right - left,
                h: top - bottom,
            }
        }
        OriginPos::TopLeft => BoxGeometry {
            x: left,
            y: top,
            w: right - left,
            h: bottom - top,
        },
    }
}
