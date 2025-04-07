use eyre::{Context, ensure};
use std::{fs, path::Path};
use teamim::parse_box_line;

/// - Add a trailing space to each line
fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let src_dir = Path::new("assets/corrected_boxfiles");
    ensure!(src_dir.is_dir(), "{src_dir:?} is not a dir");

    fs::read_dir(src_dir)?.try_for_each(|entry| {
        let entry = entry?;
        ensure!(entry.file_type()?.is_file(), "{entry:?} is not a file");
        let r = fs::read_to_string(entry.path())?;

        let lines = r.lines().enumerate().map(|(i, line)| {
            parse_box_line(line)
                .wrap_err_with(|| format!("invalid line: {}:{}", entry.path().display(), i + 1))
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
                entry.path().display(),
            );

            following_space = line.value == ' ';

            if line.value == '\t' {
                ensure!(
                    contains_letter,
                    "empty line at {}:{line_i}",
                    entry.path().display()
                );
                contains_letter = false;

                ensure!(
                    !following_space,
                    "trailing space before {}:{line_i}:{col_i}",
                    entry.path().display()
                );

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
