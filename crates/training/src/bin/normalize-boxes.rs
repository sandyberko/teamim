use eyre::{OptionExt, ensure};
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
    let files = fs::read_dir("assets/corrected_boxfiles")?
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
            for line in BufReader::new(fs::read_to_string(box_orig_path)?.as_bytes()).lines() {
                let bx = parse_box_line(&line?)?;
                if bx.value != '\n' {
                    writeln!(&mut w, "{bx}")?;
                }
            }
            eyre::Ok(())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(())
}
