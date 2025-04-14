use eyre::{Context, ensure};
use std::{ffi::OsStr, fs, path::Path};
use teamim::tesseract_ext::bounding_box::parse_char_box;

/// - Add a trailing space to each line
fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let src_dir = Path::new("assets/corrected_boxfiles");
    ensure!(src_dir.is_dir(), "{src_dir:?} is not a dir");

    fs::read_dir(src_dir)?
        .chain(fs::read_dir("../training/training/images")?)
        .try_for_each(|entry| {
            let entry = entry?;
            let path = entry.path();

            ensure!(entry.file_type()?.is_file(), "{entry:?} is not a file");
            if path.extension() != Some(OsStr::new("box")) {
                return Ok(());
            }

            let r = fs::read_to_string(&path)?;

            let lines = r.lines().enumerate().map(|(i, line)| {
                parse_char_box(line)
                    .wrap_err_with(|| format!("invalid line: {}:{}", path.display(), i + 1))
            });

            let mut following_space = false;
            let mut line_i = 1;
            let mut col_i = 1;
            let mut contains_letter = false;

            for bx in lines {
                let line = bx?;

                ensure!(
                    !(line.value == ' ' && following_space),
                    "double space at {}:{line_i}:{col_i}",
                    path.display(),
                );

                following_space = line.value == ' ';

                if line.value == '\t' {
                    // verify line

                    ensure!(contains_letter, "empty line at {}:{line_i}", path.display());
                    contains_letter = false;

                    ensure!(
                        !following_space,
                        "trailing space before {}:{line_i}:{col_i}",
                        path.display()
                    );

                    {
                        const MIN_WIDTH: i32 = 3;
                        let width = line.rect.right - line.rect.left;
                        let height = line.rect.top - line.rect.bottom;
                        ensure!(
                            width >= MIN_WIDTH && height >= MIN_WIDTH,
                            "line too small {width}x{height} at {}:{line_i}:{col_i}",
                            path.display(),
                        );
                    }

                    line_i += 1;
                    col_i = 1;
                } else {
                    contains_letter = true;

                    col_i += 1;
                }
            }
            eyre::Ok(())
        })?;
    Ok(())
}
