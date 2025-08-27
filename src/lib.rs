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
    sync::LazyLock,
};

use eyre::{OptionExt, WrapErr, bail, eyre};
use glyph::{GLYPHS, Placement};
use leptonica_ext::{Buf, PixBox as Pix};
use serde::Serialize;
use similar::{Algorithm, DiffTag, TextDiff};
use tesseract_ext::{
    PageIteratorLevel, PageSegMode, Tess, Text,
    bounding_box::{BoundingBox, Rect},
};
use thiserror::Error;
use training_diff::BoundingBoxDiff;

use crate::{
    glyph::{MAQAF, SOF_PASUQ},
    leptonica_ext::BoxGeometry,
};

const DATAPATH: &CStr = c"./assets/tessdata";
const LANG: &CStr = c"stam";

static DIACRIT_MAP: LazyLock<eyre::Result<BTreeMap<usize, (char, char)>>> =
    LazyLock::new(build_diacrit_map);

#[derive(Debug)]
pub struct DiacMiss {
    pub letter: char,
    pub diacritic: char,
    pub char_idx: usize,
    pub top: i32,
}

#[derive(Debug, Clone, Copy)]
pub enum DrawProgress {
    Recognizing,
    ImageEffects,
    Searching,
    Diffing,
    Placing,
}

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

    pub fn file_boxes(
        &mut self,
        img: impl AsRef<Path>,
    ) -> eyre::Result<(impl Iterator<Item = BoundingBox<Text>>, i32, i32)> {
        let filename = CString::new(img.as_ref().to_str().ok_or_eyre("non-utf8 path")?)?;
        let mut pix = Pix::read(&filename)?;
        self.tess.set_image(&mut pix);
        self.tess.recognize()?;

        let boxes = self.tess.results_iter(PageIteratorLevel::Textline);
        Ok((boxes, pix.get_h(), pix.get_w()))
    }

    pub fn recognize(&mut self, img: &[u8]) -> eyre::Result<String> {
        let mut pix = Pix::read_mem(img)?;
        self.tess.set_image(&mut pix);
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
    ) -> eyre::Result<(Vec<BoundingBoxDiff>, i32, i32)> {
        let mut pix = Pix::read_mem(img)?;
        self.tess.set_image(&mut pix);
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

    // TODO return something like BufPNG
    pub fn place_teamim(&mut self, img: &[u8], options: PlaceOptions) -> Result<Buf, PlaceError> {
        let mut img = Pix::read_mem(img)?;
        img = img.into_32()?;
        self.place_teamim_pix(&mut img, options, |_| ())?;
        Ok(img.copy_to_png()?)
    }

    pub fn place_teamim_pix(
        &mut self,
        img: &mut Pix,
        options: PlaceOptions,
        progress_callback: impl Fn(DrawProgress),
    ) -> Result<Vec<DiacMiss>, PlaceError> {
        progress_callback(DrawProgress::Recognizing);
        self.tess.set_image(img);
        self.tess.recognize()?;

        let snippet = self.tess.get_text()?;
        let snippet = snippet.as_str()?.replace(char::is_whitespace, "");
        let boxes = self
            .tess
            .results_iter(PageIteratorLevel::Symbol)
            .map(|BoundingBox { value, rect, page }| {
                eyre::Ok(BoundingBox {
                    value: value.as_str()?.chars().next().ok_or_eyre("empty box")?,
                    rect,
                    page,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        progress_callback(DrawProgress::ImageEffects);
        if options.blur != 0 {
            *img = img.blur(options.blur)?;
        }

        if options.contrast != 0.0 {
            img.contrast(options.contrast)?;
        }

        let options = options.estimate_scale(&boxes);

        progress_callback(DrawProgress::Searching);
        let r#match = search::approx_match(&snippet).ok_or(PlaceError::NotFound)?;
        let snip_char_offset = r#match.byte_pos / 2; // each hebrew letter is 2 bytes

        // diff
        progress_callback(DrawProgress::Diffing);
        let (old, new) = (snippet.as_str(), r#match.text);
        let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_chars(old, new);
        let mut remapper = diff.ops().iter().peekable();

        progress_callback(DrawProgress::Placing);
        let mut last_diacrit_top = 0;
        let mut misses = Vec::new();

        let snip_diacs = DIACRIT_MAP.as_ref().map_err(|e| eyre!(e))?.range(snip_char_offset..);
        'taam: for (&char_idx, &(diacritic, letter)) in snip_diacs {
            let char_offset = char_idx - snip_char_offset;
            let change = 'change: loop {
                let Some(change) = remapper.peek() else {
                    break 'taam;
                };
                if change.new_range().start > char_offset {
                    continue 'taam;
                }
                if change.new_range().end > char_offset {
                    break 'change change;
                }
                remapper.next();
            };
            match change.tag() {
                DiffTag::Equal => {
                    let char_offset_in_change = char_offset - change.new_range().start;
                    let box_idx = change.old_range().start + char_offset_in_change;
                    let bx = &boxes[box_idx];
                    last_diacrit_top = last_diacrit_top.max(bx.rect.top);
                    place_taam(
                        img,
                        options,
                        letter,
                        &into_geometry(bx, OriginPos::TopLeft),
                        diacritic,
                    )?;
                }
                DiffTag::Delete => eprintln!("  > ⚠️ DELETED this should not happen"),
                DiffTag::Insert | DiffTag::Replace => {
                    misses.push(DiacMiss { letter, diacritic, char_idx, top: last_diacrit_top });
                }
            }
        }
        Ok(misses)
    }
}

fn build_diacrit_map() -> eyre::Result<BTreeMap<usize, (char, char)>> {
    let mut byte_idx = 0;
    let mut map = BTreeMap::new();
    let teamim = include_str!("../assets/text/mam/teamim.txt");
    let mut last_letter = Option::<char>::None;
    for c_taam in teamim.chars() {
        #[expect(clippy::match_same_arms)]
        match c_taam {
            // Ta'am
            '\u{0591}'..='\u{05AD}' | '\u{5bd}' | '\u{5be}' | '\u{5c0}' | '\u{5c3}' | '\u{5c4}' => {
                // off by one, because the ta'am is placed after we encounter the letter
                map.insert(byte_idx - 1, (c_taam, last_letter.ok_or_eyre("ta'am without letter")?));
            }
            // text seems to mistakenly use tzinor instead of zarqa
            '\u{05AE}' => {
                // off by one, because the ta'am is placed after we encounter the letter
                map.insert(
                    byte_idx - 1,
                    ('\u{0598}', last_letter.ok_or_eyre("ta'am without letter")?),
                );
            }
            // Niqqud
            ('\u{05b0}'..='\u{05bc}') | '\u{05c1}' | '\u{05c2}' | '\u{05c7}' => continue,
            // Rafeh
            '\u{05bf}' => continue,
            // whitespace
            '\n' | ' ' => continue,
            // Letter - alef to tav
            ('\u{05d0}'..='\u{05EA}') => {
                last_letter = Some(c_taam);
                byte_idx += 1;
            }
            c => {
                return Err(eyre!("unexpected taaam_c: 0x{:x} {c:?}", c as u32));
            }
        }
    }
    Ok(map)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PlaceOptions {
    pub inline_diacs: bool,
    pub debug_boxes: bool,
    pub scale: f32,
    pub blur: i32,
    pub contrast: f32,
}

const FULL_WIDTH_LETTERS: &[char] =
    &['א', 'ב', 'ד', 'ה', 'ח', 'ט', 'כ', 'ל', 'מ', 'ס', 'ע', 'פ', 'צ', 'ק', 'ר', 'ש', 'ת'];

impl PlaceOptions {
    #[must_use]
    pub fn set_blur(mut self, blur: i32) -> Self {
        self.blur = blur;
        self
    }

    #[must_use]
    pub fn set_contrast(mut self, contrast: f32) -> Self {
        self.contrast = contrast;
        self
    }

    #[must_use]
    pub fn set_debug_boxes(mut self, debug_boxes: bool) -> Self {
        self.debug_boxes = debug_boxes;
        self
    }

    #[must_use]
    fn estimate_scale(mut self, boxes: &[BoundingBox<char>]) -> Self {
        let mut widths = boxes
            .iter()
            .filter(|bx| FULL_WIDTH_LETTERS.contains(&bx.value))
            .map(|bx| bx.rect.width())
            .collect::<Vec<_>>();
        widths.sort_unstable();
        let median = widths.get(widths.len() / 2).copied().unwrap_or(0);

        #[expect(clippy::cast_precision_loss)]
        {
            self.scale = median as f32 / 32.0;
        }

        self
    }
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
    img: &[u8],
    options: PlaceOptions,
    text: &str,
    boxes: impl IntoIterator<Item = eyre::Result<BoxGeometry>>,
) -> Result<Buf, PlaceError> {
    let mut img = Pix::read_mem(img)?;
    img = img.into_32()?;

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
                place_taam(&mut img, options, cur_c, cur_box, c_taam)?;
            }
            // text seems to mistakenly use tzinor instead of zarqa
            '\u{05AE}' => {
                if n > 0 {
                    continue;
                }
                let (_, cur_c) = cur_c.ok_or_eyre("expected char")?;
                let cur_box = cur_box.as_ref().ok_or_eyre("expected box")?;
                place_taam(&mut img, options, cur_c, cur_box, '\u{0598}')?;
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
    img: &mut Pix,
    options: PlaceOptions,
    cur_c: char,
    cur_box: &BoxGeometry,
    c_taam: char,
) -> Result<(), eyre::Error> {
    let Some(glyph) = GLYPHS.get(&c_taam) else {
        bail!("no glyph for {c_taam:?} {:x}", c_taam as u32);
    };

    if !options.inline_diacs && glyph.placement == Placement::After {
        return Ok(());
    }

    #[expect(clippy::cast_possible_truncation)]
    glyph
        .pix
        .try_with(|pix| {
            let mut scale_factor = options.scale;
            if glyph.placement == Placement::After {
                scale_factor *= 0.6;
            }

            // TODO don't clone
            let pix = Pix::clone(pix).scale(scale_factor)?;

            let g_margin_top = (6.0 * scale_factor) as i32;
            let margin_top = (20.0 * scale_factor) as i32;
            let (x, y) = {
                let BoxGeometry { x, y, w, h } = *cur_box;
                match glyph.placement {
                    Placement::Top => (x, y - g_margin_top - margin_top),
                    Placement::Bottom => (x, y + h - g_margin_top),
                    Placement::After => {
                        if glyph == &MAQAF {
                            let lamed = if cur_c == 'ל' { h / 2 } else { 0 };
                            (x - w - (5.0 * scale_factor) as i32, y + lamed - g_margin_top)
                        } else if glyph == &SOF_PASUQ {
                            (x - w - (3.0 * scale_factor) as i32, y - g_margin_top)
                        } else {
                            panic!("unexpected diacritic [א{c_taam}] placed after")
                        }
                    }
                }
            };

            // debug
            let w = pix.get_w();
            let h = pix.get_h();
            if options.debug_boxes {
                img.render_box(&BoxGeometry { x, y, w, h }, 2, (0, 0, 255))?;
            }

            // + h ???
            img.render_img(pix, x, y + h)
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
        let options =
            PlaceOptions { debug_boxes: true, inline_diacs: true, ..PlaceOptions::default() };
        let img = ctx.place_teamim(img, options)?;
        fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/images/N2/012.png"), img)?;
        Ok(())
    }

    #[test]
    fn test_diactrit_map() -> eyre::Result<()> {
        color_eyre::install()?;

        let map = build_diacrit_map()?;
        for (i, char) in search::text().chars().enumerate().take(100) {
            eprint!("{i}:\t{char}\t");
            if let Some((diacritic, letter)) = map.get(&i) {
                eprint!("[{letter}{diacritic}]");
            } else {
                eprint!("[ ]");
            }
            eprintln!();
        }
        Ok(())
    }
}
