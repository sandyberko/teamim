use color_eyre::Section;
use eyre::{ensure, eyre};
use itertools::Itertools;
use std::{
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader},
    path::Path,
};
use teamim::{TRAINING_TEXT, tesseract_ext::bounding_box::parse_line_boxes};

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let src_dir = Path::new("assets/corrected_boxfiles");
    ensure!(src_dir.is_dir(), "{src_dir:?} is not a dir");

    fs::read_dir(src_dir)?
        .chain(fs::read_dir("assets/training/images")?)
        .try_for_each(|entry| {
            let entry = entry?;
            let path = entry.path();
            verify_entry(&entry).map_err(|located_err| {
                located_err.err.wrap_err(format!(
                    "at {}:{}:{}",
                    path.display(),
                    located_err.row + 1,
                    located_err.col + 1
                ))
            })
        })?;
    Ok(())
}

#[derive(Debug)]
struct LocatedError {
    row: usize,
    col: usize,
    err: eyre::Report,
}

impl LocatedError {
    fn new(row: usize, col: usize, err: eyre::Report) -> Self {
        Self { row, col, err }
    }
}

impl<E: Into<eyre::Report>> From<E> for LocatedError {
    fn from(err: E) -> Self {
        LocatedError {
            row: 0,
            col: 0,
            err: err.into(),
        }
    }
}

fn verify_entry(entry: &fs::DirEntry) -> Result<(), LocatedError> {
    let path = entry.path();

    if !entry.file_type()?.is_file() {
        return Err(eyre!("not a file").into());
    }
    if path.extension() != Some(OsStr::new("box")) {
        return Ok(());
    }

    if path.file_stem().is_some_and(|stem| stem == "248_088") {
        // skip because of scribe error "הבער"
        return Ok(());
    }

    let lines = BufReader::new(fs::File::open(&path)?)
        .lines()
        .map(|res| res.map_err(From::from));

    #[expect(unstable_name_collisions)]
    let page_txt = parse_line_boxes(lines)
        .map(|bx| {
            let bx = match bx {
                Ok(bx) => bx,
                Err(e) => return Err((0, e)),
            };

            {
                const MIN_WIDTH: i32 = 3;
                let width = bx.rect.right - bx.rect.left;
                let height = bx.rect.top - bx.rect.bottom;
                if !(width >= MIN_WIDTH && height >= MIN_WIDTH) {
                    return Err((
                        0,
                        eyre!("line too small {width}x{height}, min {MIN_WIDTH}")
                            .section(format!("Text: {}", bx.value)),
                    ));
                }
            }

            if bx.value.is_empty() {
                return Err((0, eyre!("empty line")));
            }

            let mut col_idx = 0;
            for word in bx.value.split(' ') {
                let len = word.chars().count();
                if len < 2 {
                    return Err((
                        col_idx,
                        eyre!("word too short with {len}, min 2")
                            .section(format!("Text: {}", bx.value))
                            .section(format!("Page: {}", bx.page)),
                    ));
                }
                if let Some((idx, c)) = word.match_indices(|c| !('א'..='ת').contains(&c)).next() {
                    return Err((col_idx + idx, eyre!("invalid char {c:?}")));
                }
                col_idx += len;
            }

            Ok(bx.value)
        })
        .enumerate()
        .map(|(line_idx, res)| res.map_err(|(col_idx, e)| LocatedError::new(line_idx, col_idx, e)))
        .intersperse_with(|| Ok(String::from(" ")))
        .collect::<Result<String, _>>()?;

    if !TRAINING_TEXT.contains(&page_txt) {
        return Err(eyre!("text not found")
            .section(format!("Text: {page_txt:#?}"))
            .into());
    }

    Ok(())
}