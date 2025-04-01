use eyre::{Context, OptionExt, ensure};
use rayon::prelude::*;
use std::{
    fs,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};
use teamim::parse_box_line;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let out_dir = Path::new("assets/corrected_boxfiles_2");
    fs::read_dir("assets/corrected_boxfiles")?
        .map(|entry| {
            let entry = entry?;
            ensure!(entry.file_type()?.is_file(), "{entry:?} is not a file");
            Ok(entry.path())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_par_iter()
        .map(|box_orig_path| {
            let out_path = out_dir.join(box_orig_path.file_name().ok_or_eyre("no file name")?);
            let mut w = BufWriter::new(fs::File::create(out_path)?);
            let r = BufReader::new(fs::File::open(&box_orig_path)?);
            let lines = r.lines().enumerate().map(|(i, line)| {
                parse_box_line(&line?).wrap_err_with(|| {
                    format!("invalid line: {}:{}", box_orig_path.display(), i + 1)
                })
            });

            let mut has_space = false;
            for bx in lines {
                let line = bx?;

                if line.value == ' ' {
                    has_space = true;
                }

                if line.value == '\t' {
                    assert!(
                        has_space,
                        "invalid line at {}: {line}",
                        box_orig_path.display()
                    );
                    writeln!(&mut w, "{}", line.with_value(' '))?;
                }

                writeln!(&mut w, "{line}")?;
            }
            eyre::Ok(())
        })
        .collect::<Result<(), _>>()?;
    Ok(())
}
