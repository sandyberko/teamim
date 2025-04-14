use eyre::{Context, ensure};
use rayon::prelude::*;
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
};
use teamim::tesseract_ext::bounding_box::parse_char_box;

/// - Add a trailing space to each line
fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let src_dir = Path::new("assets/corrected_boxfiles");
    ensure!(src_dir.is_dir(), "{src_dir:?} is not a dir");

    fs::read_dir(src_dir)?
        .par_bridge()
        .map(|entry| {
            let entry = entry?;
            ensure!(entry.file_type()?.is_file(), "{entry:?} is not a file");
            let r = fs::read_to_string(entry.path())?;

            let mut w = BufWriter::new(
                fs::File::options()
                    .truncate(true)
                    .write(true)
                    .open(entry.path())?,
            );

            let lines = r.lines().enumerate().map(|(i, line)| {
                parse_char_box(line)
                    .wrap_err_with(|| format!("invalid line: {}:{}", entry.path().display(), i + 1))
            });

            let mut is_line_end = true;
            for bx in lines {
                let line = bx?;

                if !(is_line_end && line.value == ' ') {
                    writeln!(&mut w, "{line}")?;
                }

                is_line_end = line.value == '\t';
            }
            eyre::Ok(())
        })
        .collect::<Result<(), _>>()?;
    Ok(())
}
