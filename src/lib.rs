pub mod fuzzy_find;
pub mod glyph;
pub mod leptonica_ext;
pub mod tesseract_ext;
pub mod training_diff;

use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fmt::Write as _,
    fs,
    path::Path,
};

use eyre::{OptionExt, WrapErr, bail, eyre};
use glyph::{GLYPHS, Placement};
use leptess::leptonica::{self, BoxGeometry, Pix};
use leptonica_ext::{Buf, PixExt};
use serde::Serialize;
use similar::{Algorithm, DiffTag, TextDiff};
use tesseract_ext::{
    PageIteratorLevel, PageSegMode, Tess, Text,
    bounding_box::{BoundingBox, Rect},
};
use thiserror::Error;
use training_diff::BoundingBoxDiff;

const DATAPATH: &CStr = c"./assets/tessdata";
const LANG: &CStr = c"stam";

#[derive(Clone)]
pub struct TeamimCtx {
    tess: Tess,
}

impl TeamimCtx {
    pub fn new() -> eyre::Result<Self> {
        let tess = Tess::new(DATAPATH, LANG)?;
        tess.set_page_seg_mode(PageSegMode::SingleColumn);
        Ok(Self { tess })
    }

    pub fn with_debug_file(mut self, debug_file: impl AsRef<Path>) -> eyre::Result<Self> {
        let debug_file = debug_file.as_ref();
        let debug_file =
            debug_file.to_str().ok_or_else(|| eyre!("non-utf8 path: {debug_file:?}"))?;
        self.tess.set_variable(c"debug_file", CString::new(debug_file)?)?;
        Ok(self)
    }

    pub fn file_text(&mut self, img: impl AsRef<Path>) -> eyre::Result<Text> {
        let pix = leptonica::pix_read(img.as_ref())?;
        self.tess.set_image(&pix);
        self.tess.recognize()?;
        self.tess.get_text()
    }

    pub fn file_boxes(
        &mut self,
        img: impl AsRef<Path>,
    ) -> eyre::Result<(impl Iterator<Item = BoundingBox<Text>>, u32, u32)> {
        let pix = leptonica::pix_read(img.as_ref())?;
        self.tess.set_image(&pix);
        self.tess.recognize()?;

        let boxes = self.tess.results_iter(PageIteratorLevel::Textline);
        Ok((boxes, pix.get_h(), pix.get_w()))
    }

    pub fn recognize(&mut self, img: &[u8]) -> eyre::Result<String> {
        let pix = leptonica::pix_read_mem(img)?;
        self.tess.set_image(&pix);
        self.tess.recognize()?;

        let mut w = String::new();
        for result in self.tess.results_iter(PageIteratorLevel::Symbol) {
            writeln!(&mut w, "{result}")?;
        }
        Ok(w)
    }

    pub fn recognize_training(
        &mut self,
        img: &[u8],
    ) -> eyre::Result<(Vec<BoundingBoxDiff>, u32, u32)> {
        let pix = leptonica::pix_read_mem(img)?;
        self.tess.set_image(&pix);
        self.tess.recognize()?;

        // TODO is it already owned?
        let ocr_text = self.tess.get_text()?;
        let ocr_text = ocr_text.as_str()?.replace('\n', " ");

        let truth_text = find_truth_text(&ocr_text).ok_or_eyre("not found")?;

        let boxes = self.tess.results_iter(PageIteratorLevel::Textline).collect::<Vec<_>>();

        let boxes =
            boxes.iter().map(|bb| bb.with_value(bb.value.as_str().unwrap())).collect::<Vec<_>>();

        let diff = training_diff::diff(boxes.as_ref(), &ocr_text, truth_text);

        let w = pix.get_w();
        let h = pix.get_h();
        Ok((diff, w, h))
    }

    pub fn place_teamim(&mut self, img: &[u8], options: PlaceOptions) -> Result<Buf, PlaceError> {
        let mut img = leptonica::pix_read_mem(img)?;
        img.convert_to_32()?;

        self.tess.set_image(&img);
        self.tess.recognize()?;

        let snippet = self.tess.get_text()?;
        let snippet = snippet.as_str()?.replace(char::is_whitespace, "");
        let r#match = search::approx_match(&snippet).ok_or(PlaceError::NotFound)?;
        eprintln!("match found at {}", r#match.byte_pos);

        // diff
        let (old, new) = (snippet.as_str(), r#match.text);
        let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_chars(old, new);
        let mut remapper = diff.ops().iter();

        // map.key -> diff -> boxes[]
        let boxes = self.tess.results_iter(PageIteratorLevel::Symbol).collect::<Vec<_>>();
        let map = build_diacrit_map()?;
        let snip_char_offset = r#match.byte_pos / 2;
        for (char_idx, diacritic) in map.range(snip_char_offset..) {
            eprintln!("placing '{diacritic}' at {char_idx}");
            let char_offset = char_idx - snip_char_offset;
            let Some(change) = remapper.find(|change| change.new_range().contains(&char_offset))
            else {
                eprintln!("==== DONE");
                break;
            };
            match change.tag() {
                DiffTag::Equal => {
                    let char_offset_in_change = char_offset - change.new_range().start;
                    let box_idx = change.old_range().start + char_offset_in_change;
                    let bx = &boxes[box_idx];
                    eprintln!("  > on {}", bx.value);
                }
                DiffTag::Delete => eprintln!("  > DELETED this should not happen"),
                DiffTag::Insert => eprintln!("  > cannot place, OCR missed it"),
                DiffTag::Replace => eprintln!("  > cannot place, OCR replaced it"),
            }
        }
        Ok(img.copy_to_png()?)
    }
}

fn build_diacrit_map() -> eyre::Result<BTreeMap<usize, char>> {
    let mut byte_idx = 0;
    let mut map = BTreeMap::new();
    let teamim = include_str!("../assets/text/mam/teamim.txt");
    for c_taam in teamim.chars() {
        match c_taam {
            // Ta'am
            '\u{0591}'..='\u{05AD}' | '\u{5bd}'..='\u{5bf}' | '\u{5c0}' | '\u{5c3}' | '\u{5c4}' => {
                map.insert(byte_idx, c_taam);
            }
            // text seems to mistakenly use tzinor instead of zarqa
            '\u{05AE}' => {
                map.insert(byte_idx, '\u{0598}');
            }
            // Niqqud or whitespace
            ('\u{05b0}'..='\u{05bc}') | '\u{05c1}' | '\u{05c2}' | '\u{05c7}' | '\n' | ' ' => {
                continue;
            }
            // Letter - alef to tav
            ('\u{05d0}'..='\u{05EA}') => byte_idx += 1,
            c => {
                return Err(eyre!("unexpected taaam_c: 0x{:x} {c:?}", c as u32));
            }
        }
    }
    Ok(map)
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
    #[error("Pix error: {0}")]
    Pix(#[from] leptess::leptonica::PixError),
    #[error(transparent)]
    Other(#[from] eyre::Report),
}

pub fn place_teamim(
    img: &[u8],
    options: PlaceOptions,
    text: &str,
    boxes: impl IntoIterator<Item = eyre::Result<BoxGeometry>>,
) -> Result<Buf, PlaceError> {
    let mut img = leptonica::pix_read_mem(img)?;
    img.convert_to_32()?;

    let consonants = fs::read_to_string("./assets/text/mam/consonants/torah.txt")
        .wrap_err("failed to read consonants")?;
    let text = text.trim();
    let mut n = fuzzy_find::find(&consonants, text).ok_or(PlaceError::NotFound)?;

    let mut chars_iter = text.chars().enumerate();
    let mut cur_c: Option<(usize, char)> = None;
    let mut cur_line = 0usize;
    let mut cur_col = 0usize;
    let mut boxes_iter = boxes.into_iter();
    let mut cur_box: Option<BoxGeometry> = None;
    let teamim = fs::read_to_string("./assets/text/mam/teamim/torah.txt")
        .wrap_err("failed to read teamim")?;

    'teamim: for (_, c_taam) in teamim.char_indices() {
        match c_taam {
            // Ta'am
            '\u{0591}'..='\u{05AD}' | '\u{5bd}'..='\u{5bf}' | '\u{5c0}' | '\u{5c3}' | '\u{5c4}' => {
                if n > 0 {
                    continue;
                }
                // should be this, but doesn't work after skipping
                // let (_, cur_c) = cur_c.ok_or_eyre("expected char")?;
                let Some((_, cur_c)) = cur_c else {
                    continue 'teamim;
                };
                let cur_box = cur_box.as_ref().ok_or_eyre("expected box")?;
                place_taam(&img, options, cur_c, cur_box, c_taam)?;
            }
            // text seems to mistakenly use tzinor instead of zarqa
            '\u{05AE}' => {
                if n > 0 {
                    continue;
                }
                let (_, cur_c) = cur_c.ok_or_eyre("expected char")?;
                let cur_box = cur_box.as_ref().ok_or_eyre("expected box")?;
                place_taam(&img, options, cur_c, cur_box, '\u{0598}')?;
            }
            '\n' => {
                cur_line += 1;
                cur_col = 0;
                continue;
            }
            // Niqqud
            ('\u{05b0}'..='\u{05bc}') | '\u{05c1}' | '\u{05c2}' | '\u{05c7}' => continue,
            _ if c_taam.is_whitespace() => continue,
            // Letter - alef to tav
            ('\u{05d0}'..='\u{05EA}') => {
                cur_col += 1;
                if n > 0 {
                    n -= 1;
                    continue;
                }
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
                .into());
            }
        }
    }
    Ok(img.copy_to_png()?)
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
            let scale_factor = if glyph.placement == Placement::After { 0.4 } else { 0.5 };
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

#[derive(Copy, Clone, Debug)]
pub enum OriginPos {
    BottomLeft { img_h: u32 },
    TopLeft,
}

#[must_use]
pub fn into_geometry<V>(bx: &BoundingBox<V>, origin_pos: OriginPos) -> BoxGeometry {
    let Rect { left, bottom, right, top } = bx.rect;
    match origin_pos {
        OriginPos::BottomLeft { img_h } => {
            let img_h: i32 = img_h.try_into().unwrap();
            BoxGeometry { x: left, y: img_h - top, w: right - left, h: top - bottom }
        }
        OriginPos::TopLeft => BoxGeometry { x: left, y: top, w: right - left, h: bottom - top },
    }
}

#[derive(Serialize)]
pub enum DiffOp<'s> {
    Insert { new_index: usize, new_len: usize },
    Delete { new_index: usize, old: &'s str },
    Replace { new_index: usize, new_len: usize, old: &'s str },
}

pub const TRAINING_TEXT: &str = include_str!("../assets/text/mam/training.txt");
pub fn diff(text: &str) -> eyre::Result<Vec<DiffOp<'static>>> {
    let old = find_truth_text(text).ok_or_eyre("not found")?;
    Ok(TextDiff::from_graphemes(old, text)
        .ops()
        .iter()
        .filter_map(|op| match *op {
            similar::DiffOp::Delete { new_index, .. } => {
                Some(DiffOp::Delete { new_index, old: "" })
            }
            similar::DiffOp::Insert { new_index, new_len, .. } => {
                Some(DiffOp::Insert { new_index, new_len })
            }
            similar::DiffOp::Equal { .. } => None,
            similar::DiffOp::Replace { new_index, new_len, .. } => {
                Some(DiffOp::Replace { new_index, new_len, old: "" })
            }
        })
        .collect())
}

#[must_use]
pub fn find_truth_text(ocr_text: &str) -> Option<&str> {
    let position = {
        let snippet = if let Some((idx, _)) = ocr_text.char_indices().nth(25) {
            &ocr_text[..idx]
        } else {
            ocr_text
        };
        TRAINING_TEXT.find(snippet)?
    };
    let old = &TRAINING_TEXT[position..];
    let mut end = ocr_text.len();
    while !old.is_char_boundary(end) {
        end += 1;
    }
    Some(&old[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_place_teamim() -> eyre::Result<()> {
        color_eyre::install()?;

        let mut ctx = TeamimCtx::new()?;
        let img = include_bytes!("../assets/images/N2/012.jpg");
        let options = PlaceOptions { enable_after: true, debug_boxes: false };
        ctx.place_teamim(img, options)?;
        Ok(())
    }
}
