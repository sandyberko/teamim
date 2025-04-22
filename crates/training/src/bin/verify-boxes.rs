use clap::Parser;
use eyre::{ensure, eyre};
use itertools::Itertools;
use std::{
    ffi::OsStr,
    fmt::Display,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
use teamim::{TRAINING_TEXT, tesseract_ext::bounding_box::parse_line_boxes};

#[derive(Debug, Parser)]
struct Args {
    file: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    if let Some(path) = args.file {
        verify_file(&path).map_err(|err| FiledError::new(path, err))?;
        return Ok(());
    }

    let src_dir = Path::new("assets/corrected_boxfiles");
    ensure!(src_dir.is_dir(), "{src_dir:?} is not a dir");

    fs::read_dir(src_dir)?.chain(fs::read_dir("assets/training/images")?).try_for_each(
        |entry| {
            let entry = entry?;
            let path = entry.path();
            verify_file(&path).map_err(|err| FiledError::new(path, err))?;
            eyre::Ok(())
        },
    )?;
    Ok(())
}

#[derive(Debug)]
struct FiledError {
    path: PathBuf,
    err: LocatedError,
}

impl FiledError {
    fn new(path: PathBuf, err: LocatedError) -> Self {
        Self { path, err }
    }
}

impl std::error::Error for FiledError {}

impl Display for FiledError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let LocatedError { row, col, err } = &self.err;
        write!(f, "{}:{}:{}: {err}", self.path.display(), row + 1, col + 1)
    }
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
        LocatedError { row: 0, col: 0, err: err.into() }
    }
}

fn verify_file(path: &Path) -> Result<(), LocatedError> {
    if path.extension() != Some(OsStr::new("box")) {
        return Ok(());
    }

    if path.file_stem().is_some_and(|stem| stem == "248_088") {
        // skip because of scribe error "הבער"
        return Ok(());
    }

    let lines = BufReader::new(fs::File::open(path)?).lines().map(|res| res.map_err(From::from));

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
                    return Err((0, eyre!("line too small {width}x{height}, min {MIN_WIDTH}")));
                }
            }

            if bx.value.is_empty() {
                return Err((0, eyre!("empty line")));
            }

            let mut col_idx = 0;
            for word in bx.value.split(' ') {
                let len = word.chars().count();
                // ה לה' תגמלו זאת
                if len < 2 && word != "ה" {
                    return Err((
                        col_idx,
                        eyre!(
                            "page {}: word {word:?} too short with {len}, min 2\nline: {:?}",
                            bx.page,
                            bx.value
                        ),
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
        let txt_path = path.with_extension("txt");
        fs::write(&txt_path, &page_txt)?;

        return Err(eyre!("text not found (wrote to {txt_path:?})").into());
    }

    Ok(())
}
