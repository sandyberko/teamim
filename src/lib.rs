pub mod diac;
pub mod entire_book;
pub mod fuzzy_find;
pub mod glyph;
pub mod tesseract_ext;
pub mod training_diff;

#[cfg(any(feature = "test_utils", test))]
pub mod test_utils;

#[cfg(test)]
mod tests;

use {
    eyre::{OptionExt, ensure, eyre},
    image::{ImageBuffer, Rgba},
    serde::Serialize,
    similar::{Algorithm, DiffTag, TextDiff, udiff::UnifiedDiff, utils::TextDiffRemapper},
    std::{
        borrow::Cow,
        collections::BTreeMap,
        ffi::{CStr, CString},
        fs,
        ops::Deref,
        path::Path,
        sync::{Arc, LazyLock},
    },
    tesseract_ext::{
        PageIteratorLevel, PageSegMode, Tess, Text,
        bounding_box::{BoundingBox, Rect},
    },
    thiserror::Error,
};

pub const DATAPATH: &CStr = c"./assets/tessdata";
const LANG: &CStr = c"stam";

static DIACRIT_MAP: LazyLock<eyre::Result<BTreeMap<usize, (char, char)>>> =
    LazyLock::new(build_diacrit_map);

#[derive(Debug, Clone, Copy, Default)]
pub enum PositStatus {
    #[default]
    Pending,
    Recognizing,
    ImageEffects,
    Searching,
    Diffing,
    Placing,
    Rendering,
}

#[derive(Clone)]
pub struct TeamimCtx {
    tess: Tess,
}

impl TeamimCtx {
    #[tracing::instrument]
    pub fn new(datapath: &CStr) -> eyre::Result<Self> {
        tracing::info!("initializing context...");
        ensure!(fs::exists(datapath.to_str()?)?, "tessdata doesn't exist!");
        let mut tess = Tess::new(datapath, LANG)?;
        // TODO
        if !fs::exists("./logs")? {
            fs::create_dir("./logs")?;
        }
        tess.set_variable(c"debug_file", c"./logs/tesseract.log")?;
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

    /// # Returns
    /// (file boxes, width, height)
    pub fn file_boxes(
        &mut self,
        img_path: impl AsRef<Path>,
    ) -> eyre::Result<(impl Iterator<Item = BoundingBox<Text>>, i32, i32)> {
        let img = image::open(img_path)?.to_rgba8();
        let (width, height) = img.dimensions();
        self.tess.set_image(&img);
        self.tess.recognize()?;

        let boxes = self.tess.results_iter(PageIteratorLevel::Textline);
        Ok((boxes, width.try_into().unwrap(), height.try_into().unwrap()))
    }

    pub fn positions(
        &mut self,
        img: &ImageBuffer<Rgba<u8>, impl Deref<Target = [u8]>>,
        progress_callback: impl Fn(PositStatus),
    ) -> Result<Vec<PlacedDiac>, PlaceError> {
        progress_callback(PositStatus::Recognizing);
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

        progress_callback(PositStatus::Searching);
        let r#match = search::approx_match(&snippet).ok_or(PlaceError::NotFound)?;
        let snip_char_offset = r#match.byte_pos / 2; // each hebrew letter is 2 bytes

        // diff
        progress_callback(PositStatus::Diffing);
        // (ocr, ground_truth)
        let (old, new) = (snippet.as_str(), r#match.text);
        let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_chars(old, new);
        let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);

        progress_callback(PositStatus::Placing);
        place(&boxes, snip_char_offset, new, &remapper, diff.ops().iter().copied())
    }

    pub fn diff_boxes(
        &mut self,
        img: &ImageBuffer<Rgba<u8>, impl Deref<Target = [u8]>>,
        progress_callback: impl Fn(PositStatus),
    ) -> Result<Vec<BoxDiffOp>, PlaceError> {
        progress_callback(PositStatus::Recognizing);
        self.tess.set_image(img);
        self.tess.recognize()?;

        let snippet = self.tess.get_text()?;
        let snippet = snippet.as_str()?.replace(char::is_whitespace, "");
        let boxes = self
            .tess
            .results_iter(PageIteratorLevel::Symbol)
            .map(|bx| eyre::Ok(bx.rect))
            .collect::<Result<Vec<_>, _>>()?;

        progress_callback(PositStatus::Searching);
        let r#match = search::approx_match(&snippet).ok_or(PlaceError::NotFound)?;
        // diff
        progress_callback(PositStatus::Diffing);
        // (ocr, ground_truth)
        let (old, new) = (snippet.as_str(), r#match.text);
        let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_chars(old, new);
        {
            // Debug
            eprintln!("==========================");
            let diff = UnifiedDiff::from_text_diff(&diff);
            eprintln!("{diff}");
            eprintln!("==========================");
        }
        let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
        let ops = diff.ops().iter().peekable();

        progress_callback(PositStatus::Placing);
        let mut box_diff = Vec::new();
        for op in ops {
            match op.tag() {
                // Match!
                DiffTag::Equal => {
                    box_diff.extend(boxes[op.new_range()].iter().copied().map(BoxDiffOp::Box));
                }
                // OCR missed
                DiffTag::Replace | DiffTag::Insert => box_diff.push(BoxDiffOp::Miss(
                    remapper
                        .slice_new(op.new_range())
                        .ok_or_eyre("invalid range")?
                        .to_string()
                        .into(),
                )),
                // OCR hallucinated
                DiffTag::Delete => {}
            }
        }

        Ok(box_diff)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedDiac {
    pub letter: char,
    pub diacritic: char,
    pub place: diac::Result,
}

impl PlacedDiac {
    #[must_use]
    pub const fn new(letter: char, diacritic: char, place: diac::Result) -> Self {
        Self { letter, diacritic, place }
    }
}

///
/// # Arguments
/// - `snip_char_offset`
pub fn place(
    boxes: &[BoundingBox<char>],
    snip_char_offset: usize,
    new: &str,
    remapper: &TextDiffRemapper<'_, str>,
    ops: impl IntoIterator<Item = similar::DiffOp>,
) -> Result<Vec<PlacedDiac>, PlaceError> {
    let mut ops = ops.into_iter().peekable();

    let mut last_diacrit_top = 0;
    let mut res = Vec::new();
    let snip_diacs = DIACRIT_MAP.as_ref().map_err(|e| eyre!(e))?.range(snip_char_offset..);
    'taam: for (&char_idx, &(diacritic, letter)) in snip_diacs {
        let char_offset = char_idx - snip_char_offset;
        let change = 'change: loop {
            let Some(change) = ops.peek() else {
                break 'taam;
            };
            if change.new_range().start > char_offset {
                continue 'taam;
            }
            if change.new_range().end > char_offset {
                break 'change change;
            }
            ops.next();
        };
        let result = match change.tag() {
            DiffTag::Equal => {
                let char_offset_in_change = char_offset - change.new_range().start;
                let box_idx = change.old_range().start + char_offset_in_change;
                let rect = boxes[box_idx].rect;
                last_diacrit_top = rect.top;
                Ok(rect)
            }
            DiffTag::Delete => panic!("  > ⚠️ DELETED this should not happen"),
            DiffTag::Insert | DiffTag::Replace => {
                let range = change.new_range();
                let pre = remapper
                    .slice_new(range.start.saturating_sub(6)..range.end)
                    .ok_or_eyre("invalid range")?;
                let post = remapper
                    .slice_new(range.end..new.len().min(range.end + 6))
                    .ok_or_eyre("invalid range")?;
                let missing_text = Cow::Owned(format!("{pre}{diacritic}{post}"));
                let top = last_diacrit_top;
                Err(diac::DiacMiss { top, missing_text })
            }
        };
        res.push(PlacedDiac::new(letter, diacritic, result));
    }
    Ok(res)
}

#[derive(Debug, Clone)]
pub enum BoxDiffOp {
    Box(Rect<u32>),
    Miss(Arc<str>),
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
